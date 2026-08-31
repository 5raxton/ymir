//! Lua config engine.
//!
//! Each evaluation gets a fresh `mlua` VM with a
//! sandboxed prelude (`ymir.*` API table, tracked `include_config`/`dofile`/`require`), runs the
//! config chunk, and folds the resulting data-table (or the imperative `ymir.*` calls) into the
//! shared `Config` accumulator through the runtime `*Part` types and their `MergeWith` impls.
//!
//! Two top-level forms are accepted and transparently reconciled:
//! - a data-table return: `return { binds = { ... }, layout = { ... } }`
//! - imperative calls: `ymir.bind(...)`, `ymir.set_layout_defaults(...)`, `ymir.input { ... }`,
//!   with an optional trailing `return { ... }` merged on top.
//!
//! Errors are accumulated (validation diagnostics per `section.key`, include failures) rather than
//! fail-fast. A chunk that raises a Lua runtime
//! error aborts the whole program and that error becomes the main error.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use mlua::{Function, Lua, LuaString, MultiValue, Table, Value};
use tracing::warn;
use ymir_ipc::{
    ColumnDisplay, ConfiguredMode, LayoutSwitchTarget, SizeChange, SplitDirection, Transform,
};

use crate::animations::*;
use crate::binds::{SwitchAction, SwitchBinds};
use crate::debug::{DebugPart, PreviewRender};
use crate::gestures::{DndEdgeViewScrollPart, DndEdgeWorkspaceSwitchPart, GesturesPart, HotCorners};
use crate::input::{
    AccelProfile, ClickMethod, FocusFollowsMouse, InputPart, KeyboardPart, Mouse, ScrollFactor,
    ScrollMethod, Tablet, TapButtonMap, Touch, Touchpad, TrackLayout, Trackball, Trackpoint,
    WarpMouseToFocus, Xkb,
};
use crate::output::{MaxBpc as ConfigMaxBpc, Mode, Modeline};
use crate::recent_windows::{
    MruAction, MruBind, MruBinds, MruDirection, MruFilter, MruHighlightPart, MruPreviewsPart,
    MruScope, RecentWindowsPart,
};
use crate::error::{ConfigError, ConfigIncludeError};
use crate::utils::{Flag, FloatOrInt, MergeWith, Percent, RegexEq};
use crate::window_rule::Match as WindowMatch;
use crate::workspace::{WorkspaceLayoutPart, WorkspaceName};
use crate::*;

const RECURSION_LIMIT: u8 = 10;

/// One config file currently being evaluated.
#[derive(Debug, Clone)]
struct Frame {
    /// Directory that relative includes resolve against.
    base: PathBuf,
    /// Path used to derive `include_config` resolution and error attribution.
    path: PathBuf,
    /// Recursion depth of this file (0 for the main config).
    recursion: u8,
    /// Sections seen so far in this file, for duplicate-section detection.
    seen: HashSet<String>,
    /// Include stack for recursion detection, including this file's path.
    include_stack: HashSet<PathBuf>,
}

/// Shared parsing state. Behind `Arc<Mutex<>>` so it can be captured by `mlua` callbacks when the
/// `mlua/send` feature enables `Send + Sync` bounds.
#[derive(Debug, Default)]
struct ParseCtx {
    config: Config,
    root_base: PathBuf,
    frames: Vec<Frame>,
    includes: Vec<PathBuf>,
    include_errors: Vec<ConfigError>,
    /// Validation diagnostics: `(file path, "section.key: message")` pairs.
    validation: Vec<(PathBuf, String)>,
    saw_mru_binds: bool,
}

impl ParseCtx {
    fn current(&self) -> &Frame {
        self.frames.last().expect("ctx always has at least one frame")
    }

    fn current_mut(&mut self) -> &mut Frame {
        self.frames.last_mut().expect("ctx always has at least one frame")
    }

    fn path(&self) -> &Path {
        &self.current().path
    }

    fn push_frame(&mut self, frame: Frame) {
        self.frames.push(frame);
    }

    fn pop_frame(&mut self) {
        self.frames.pop();
    }

    fn record_validation(&mut self, key: &str, msg: impl std::fmt::Display) {
        self.validation
            .push((self.path().to_path_buf(), format!("{key}: {msg}")));
    }

    fn record_include_error(&mut self, path: impl Into<PathBuf>, message: impl Into<String>) {
        self.include_errors
            .push(ConfigError::runtime(path, message));
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Nil => "nil",
        Value::Boolean(_) => "boolean",
        Value::Integer(_) => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Table(_) => "table",
        Value::Function(_) => "function",
        Value::UserData(_) => "userdata",
        Value::Thread(_) => "thread",
        Value::LightUserData(_) => "light userdata",
        Value::Error(_) => "error",
        Value::Other(_) => "other",
    }
}

/// Normalize a config key: kebab-case and snake_case are both accepted.
fn normalize_key(key: &str) -> String {
    key.replace('-', "_")
}

/// Render a Lua string as Rust text, falling back to `""` when it is not valid UTF-8.
fn lua_str(s: &LuaString) -> String {
    s.to_str().map(|s| s.to_string()).unwrap_or_default()
}

/// Run a config file and return the resulting config together with the set of include paths.
///
/// Returns `Ok((config, includes))` on success, or `Err((main, includes))` where `main` carries
/// the primary error and `includes` carries the include-path set (still populated to feed the
/// watcher even when parsing fails).
pub fn run_program(
    path: &Path,
    text: &str,
) -> Result<(Config, Vec<PathBuf>), (ConfigIncludeError, Vec<PathBuf>)> {
    let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let filename = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("init.lua");
    let display_path = relative_to_root(path, &base)
        .unwrap_or_else(|| PathBuf::from(filename));

    let ctx = Arc::new(Mutex::new(ParseCtx {
        root_base: base.clone(),
        frames: vec![Frame {
            base,
            path: display_path.clone(),
            recursion: 0,
            seen: HashSet::new(),
            include_stack: HashSet::from([path.to_path_buf()]),
        }],
        ..Default::default()
    }));

    let lua = Lua::new();
    install_prelude(&lua, ctx.clone()).map_err(|e| {
        (
            ConfigIncludeError {
                main: ConfigError::runtime(path, e.to_string()),
                includes: Vec::new(),
            },
            Vec::new(),
        )
    })?;

    let result = lua
        .load(text)
        .set_name(display_path.display().to_string())
        .eval::<Value>();

    match result {
        Ok(value) => {
            match value {
                Value::Nil => {}
                Value::Table(table) => apply_data_table(&ctx, &table),
                other => {
                    ctx.lock().unwrap().record_validation(
                        "config",
                        format!(
                            "config file must return a table or nothing, got {}",
                            type_name(&other)
                        ),
                    );
                }
            }

            let includes = ctx.lock().unwrap().includes.clone();
            match finish(&ctx) {
                Ok(config) => Ok((config, includes)),
                Err(main) => Err((main, includes)),
            }
        }
        Err(err) => {
            // A chunk that raises aborts the program; the error is the main error.
            let mut ctx = ctx.lock().unwrap();
            let includes = ctx.includes.clone();
            let mut include_errors = std::mem::take(&mut ctx.include_errors);
            include_errors.extend(validation_errors(&ctx));
            Err((
                ConfigIncludeError {
                    main: ConfigError::runtime(path, format_lua_error(&err)),
                    includes: include_errors,
                },
                includes,
            ))
        }
    }
}

/// Produce the final result: the config if there were no errors, otherwise the main error with any
/// include errors chained.
fn finish(ctx: &Arc<Mutex<ParseCtx>>) -> Result<Config, ConfigIncludeError> {
    let mut ctx = ctx.lock().unwrap();
    let mut all_errors = std::mem::take(&mut ctx.include_errors);
    all_errors.extend(validation_errors(&ctx));

    if all_errors.is_empty() {
        Ok(std::mem::take(&mut ctx.config))
    } else {
        let main = all_errors.remove(0);
        Err(ConfigIncludeError {
            main,
            includes: all_errors,
        })
    }
}

fn validation_errors(ctx: &ParseCtx) -> Vec<ConfigError> {
    // Group validation messages by path, in first-seen order.
    let mut order: Vec<PathBuf> = Vec::new();
    let mut grouped: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for (path, msg) in &ctx.validation {
        if let Some(group) = grouped.iter_mut().find(|(p, _)| p == path) {
            group.1.push(msg.clone());
        } else {
            order.push(path.clone());
            grouped.push((path.clone(), vec![msg.clone()]));
        }
    }
    order
        .into_iter()
        .map(|path| {
            let msgs = grouped
                .iter()
                .find(|(p, _)| *p == path)
                .map(|(_, msgs)| msgs.clone())
                .unwrap_or_default();
            ConfigError::validation(path, msgs)
        })
        .collect()
}

fn format_lua_error(err: &mlua::Error) -> String {
    err.to_string()
}

fn relative_to_root(path: &Path, base: &Path) -> Option<PathBuf> {
    path.strip_prefix(base).ok().map(Path::to_path_buf)
}

/// Apply a returned data-table: dispatch every `section = value` pair through `apply_section`.
fn apply_data_table(ctx: &Arc<Mutex<ParseCtx>>, table: &Table) {
    let iter = table.pairs::<String, Value>();
    for kv in iter {
        let (key, value) = match kv {
            Ok(kv) => kv,
            Err(_) => continue,
        };
        apply_section(ctx, &key, &value);
    }
}

/// Dispatch a single top-level config section (either from a returned table or a `ymir.<section>`
/// call) into the shared `Config` accumulator.
fn apply_section(ctx: &Arc<Mutex<ParseCtx>>, section: &str, value: &Value) {
    let mut guard = ctx.lock().unwrap();
    let key = normalize_key(section);

    // Duplicate-section detection within one file. The multipart sections are exempt (they add
    // new values).
    if !is_multipart(&key) && !guard.current_mut().seen.insert(key.clone()) {
        guard.record_validation(section, "duplicate section, section already specified");
        return;
    }

    let path = guard.path().to_path_buf();
    let recursion = guard.current().recursion;

    let errors: Vec<String> = {
        let ParseCtx {
            config,
            saw_mru_binds,
            ..
        } = &mut *guard;
        let mut errors = Vec::new();

        match key.as_str() {
            "input" => apply_input(config, &key, value, &mut errors),
            "cursor" => apply_cursor(config, &key, value, &mut errors),
            "clipboard" => apply_clipboard(config, &key, value, &mut errors),
            "hotkey_overlay" => apply_hotkey_overlay(config, &key, value, &mut errors),
            "config_notification" => apply_config_notification(config, &key, value, &mut errors),
            "animations" => apply_animations(config, &key, value, &mut errors),
            "blur" => apply_blur(config, &key, value, &mut errors),
            "gestures" => apply_gestures(config, &key, value, &mut errors),
            "overview" => apply_overview(config, &key, value, &mut errors),
            "xwayland_satellite" => apply_xwayland_satellite(config, &key, value, &mut errors),
            "switch_events" => apply_switch_events(config, &key, value, &mut errors),
            "debug" => apply_debug(config, &key, value, &mut errors),
            "layout" => apply_layout(config, &key, value, &mut errors, recursion),
            "output" => apply_outputs(config, &key, value, &mut errors),
            "spawn_at_startup" => apply_spawn_at_startup(config, &key, value, &mut errors),
            "spawn_sh_at_startup" => apply_spawn_sh_at_startup(config, &key, value, &mut errors),
            "window_rules" => apply_window_rules(config, &key, value, &mut errors),
            "layer_rules" => apply_layer_rules(config, &key, value, &mut errors),
            "workspaces" => apply_workspaces(config, &key, value, &mut errors),
            "binds" => apply_binds(config, &key, value, &mut errors),
            "recent_windows" => {
                apply_recent_windows(config, &key, value, &mut errors, saw_mru_binds)
            }
            "prefer_no_csd" => apply_prefer_no_csd(config, &key, value, &mut errors),
            "screenshot_path" => apply_screenshot_path(config, &key, value, &mut errors),
            "environment" => apply_environment(config, &key, value, &mut errors),
            _ => push_err(&mut errors, section, format!("unknown section `{section}`")),
        }

        errors
    };

    for err in errors {
        guard.validation.push((path.clone(), err));
    }
}

/// Sections that can appear more than once within one file; each adds new values.
fn is_multipart(key: &str) -> bool {
    matches!(
        key,
        "output" | "spawn_at_startup" | "spawn_sh_at_startup" | "window_rules" | "layer_rules" | "workspaces"
    )
}

/// Install the sandboxed prelude into a fresh Lua state.
fn install_prelude(lua: &Lua, ctx: Arc<Mutex<ParseCtx>>) -> mlua::Result<()> {
    // Wrapped `include_config`.
    let include = make_include_config(lua, ctx.clone())?;
    lua.globals().set("include_config", include)?;

    // Tracked dofile / require / loadfile.
    lua.globals().set("dofile", make_dofile(lua, ctx.clone())?)?;
    lua.globals().set("loadfile", make_loadfile(lua, ctx.clone())?)?;
    lua.globals().set("require", make_require(lua, ctx.clone())?)?;

    // `ymir` API table.
    let ymir_t = lua.create_table()?;
    make_section_api(lua, &ymir_t, ctx.clone())?;

    let action = make_action_proxy(lua)?;
    ymir_t.set("action", action)?;

    let ctx = ctx.clone();
    let bind_fn = {
        let value = ctx.clone();
        lua.create_function(move |_lua, (key, opts): (String, Table)| {
            let ctx = value.clone();
            apply_single_bind(&ctx, &key, &opts)
        })?
    };
    ymir_t.set("bind", bind_fn)?;

    let ctx2 = ctx.clone();
    let set_layout_defaults = lua.create_function(move |_lua, t: Table| {
        let ctx = ctx2.clone();
        apply_set_layout_defaults(&ctx, &t)
    })?;
    ymir_t.set("set_layout_defaults", set_layout_defaults)?;

    // Matcher rule/hook API needs the runtime thread, which is a follow-up.
    let ctx3 = ctx.clone();
    let window_rule = lua.create_function(move |_lua, _: Table| -> mlua::Result<Value> {
        let ctx = ctx3.clone();
        let mut g = ctx.lock().unwrap();
        g.record_validation(
            "window_rule",
            "matcher window rules are not supported yet; use the `window_rules` data-table instead",
        );
        Err(mlua::Error::RuntimeError(
            "ymir.window_rule is not supported yet".to_string(),
        ))
    })?;
    ymir_t.set("window_rule", window_rule)?;

    let ctx4 = ctx.clone();
    let layer_rule = lua.create_function(move |_lua, _: Table| -> mlua::Result<Value> {
        let ctx = ctx4.clone();
        let mut g = ctx.lock().unwrap();
        g.record_validation(
            "layer_rule",
            "matcher layer rules are not supported yet; use the `layer_rules` data-table instead",
        );
        Err(mlua::Error::RuntimeError(
            "ymir.layer_rule is not supported yet".to_string(),
        ))
    })?;
    ymir_t.set("layer_rule", layer_rule)?;

    let ctx5 = ctx.clone();
    let on = lua.create_function(move |_lua, _: Table| -> mlua::Result<Value> {
        let ctx = ctx5.clone();
        let mut g = ctx.lock().unwrap();
        g.record_validation("on", "event hooks are not supported yet");
        Err(mlua::Error::RuntimeError(
            "ymir.on is not supported yet".to_string(),
        ))
    })?;
    ymir_t.set("on", on)?;

    // A null sentinel for expressing "explicitly unset" in data tables, where `nil` cannot be
    // stored (e.g. `environment = { DISPLAY = ymir.null }`).
    ymir_t.set("null", Value::NULL)?;

    lua.globals().set("ymir", ymir_t)?;

    Ok(())
}

/// Install the per-section functions (`ymir.input { ... }`, `ymir.layout { ... }`, ...) on the
/// `ymir` table.
fn make_section_api(lua: &Lua, ymir_t: &Table, ctx: Arc<Mutex<ParseCtx>>) -> mlua::Result<()> {
    let sections = [
        "input",
        "cursor",
        "clipboard",
        "hotkey_overlay",
        "config_notification",
        "animations",
        "blur",
        "gestures",
        "overview",
        "xwayland_satellite",
        "switch_events",
        "debug",
        "layout",
        "output",
        "spawn_at_startup",
        "spawn_sh_at_startup",
        "window_rules",
        "layer_rules",
        "workspaces",
        "binds",
        "recent_windows",
        "prefer_no_csd",
        "screenshot_path",
        "environment",
    ];

    for section in sections {
        let ctx = ctx.clone();
        let section = section.to_string();
        let set_key = section.clone();
        let f = lua.create_function(move |_lua, args: MultiValue| {
            let ctx = ctx.clone();
            let mut args = args.into_iter();
            let first = args.next();
            apply_section_imperative(&ctx, &section, first)
        })?;
        ymir_t.set(set_key, f)?;
    }

    Ok(())
}

fn apply_section_imperative(
    ctx: &Arc<Mutex<ParseCtx>>,
    section: &str,
    value: Option<Value>,
) -> mlua::Result<Value> {
    let value = match value {
        Some(value) => value,
        // `ymir.layout()` with no args is a no-op (empty section).
        None => return Ok(Value::Nil),
    };

    // Scalar sections (`ymir.prefer_no_csd(true)`, `ymir.screenshot_path("...")`) pass their
    // value through directly.
    if matches!(section, "prefer_no_csd" | "screenshot_path") {
        apply_section(ctx, section, &value);
        return Ok(Value::Nil);
    }

    // `ymir.output("eDP-1", { ... })` passes the name as a leading string argument; that
    // imperative named-output form is a no-op for now (use the data-table `output` form).
    if section == "output" && matches!(value, Value::String(_)) {
        return Ok(Value::Nil);
    }

    match value {
        Value::Table(t) => {
            apply_section(ctx, section, &Value::Table(t));
            Ok(Value::Nil)
        }
        other => {
            let mut g = ctx.lock().unwrap();
            g.record_validation(
                section,
                format!("expected a table, got {}", type_name(&other)),
            );
            Ok(Value::Nil)
        }
    }
}

/// `ymir.action.<kebab-name>({ args... })` returns a table `{ name = "<kebab-name>", ... }`.
fn make_action_proxy(lua: &Lua) -> mlua::Result<Table> {
    let action_t = lua.create_table()?;
    let meta = lua.create_table()?;
    let index = lua.create_function(|lua, key: String| {
        let name = key;
        let f = lua.create_function(move |lua, args: MultiValue| {
            let t = lua.create_table()?;
            t.set("name", name.clone())?;
            for arg in args.into_iter() {
                if let Value::Table(arg) = arg {
                    let iter = arg.pairs::<Value, Value>();
                    for kv in iter {
                        let (k, v) = kv?;
                        t.set(k, v)?;
                    }
                }
            }
            Ok(Value::Table(t))
        })?;
        Ok(Value::Function(f))
    })?;
    meta.set("__index", index)?;
    action_t.set_metatable(Some(meta))?;
    Ok(action_t)
}

/// `ymir.bind("Mod+Q", { action = { name = "close-window" } })` and friends, where `action` may
/// also be an `ymir.action.<name>`-style table `{ name = ..., ... }`.
fn apply_single_bind(ctx: &Arc<Mutex<ParseCtx>>, key: &str, opts: &Table) -> mlua::Result<Value> {
    let mut errors: Vec<String> = Vec::new();
    let Some(bind) = parse_bind(key, opts, &mut errors) else {
        let mut guard = ctx.lock().unwrap();
        for err in errors {
            guard.record_validation(key, err);
        }
        return Ok(Value::Nil);
    };

    let mut guard = ctx.lock().unwrap();
    for err in errors {
        guard.record_validation(key, err);
    }

    // Replace-by-key, like the `binds` section.
    guard
        .config
        .binds
        .0
        .retain(|existing| existing.key != bind.key);
    guard.config.binds.0.push(bind);

    Ok(Value::Nil)
}

/// Parse a single bind from a `{ key = "Mod+Q", action = { name = ... }, ... }`-style table.
/// `key_hint` is the key from an imperative `ymir.bind("Mod+Q", ...)` call; the table's own `key`
/// field takes precedence when present.
fn parse_bind(key_hint: &str, opts: &Table, errors: &mut Vec<String>) -> Option<Bind> {
    let key_str = match opts_raw_get(opts, "key") {
        Value::String(s) => match s.to_str().as_deref() {
            Ok(s) => s.to_string(),
            Err(_) => key_hint.to_string(),
        },
        _ => key_hint.to_string(),
    };

    let action = match opts_raw_get(opts, "action") {
        Value::Table(t) => match resolve_action(&t, errors) {
            Ok(action) => action,
            Err(()) => return None,
        },
        other => {
            errors.push(format!(
                "bind action must be a table like `{{ name = \"...\" }}`, got {}",
                type_name(&other)
            ));
            return None;
        }
    };

    let parsed_key = match key_str.trim().parse::<Key>() {
        Ok(k) => k,
        Err(err) => {
            errors.push(format!("{err}"));
            return None;
        }
    };

    let bind = Bind {
        key: parsed_key,
        action,
        repeat: read_flag_default(&opts_raw_get(opts, "repeat"), true),
        cooldown: read_duration_ms(opts, "cooldown_ms", errors),
        allow_when_locked: read_flag_default(&opts_raw_get(opts, "allow_when_locked"), false),
        allow_inhibiting: read_flag_default(&opts_raw_get(opts, "allow_inhibiting"), true),
        hotkey_overlay_title: read_hotkey_overlay_title(opts, errors),
    };

    Some(bind)
}

/// Resolve an action table into a `binds::Action`.
fn resolve_action(action: &Table, errors: &mut Vec<String>) -> Result<Action, ()> {
    let Some(name) = read_required_string(action, "name", errors) else {
        return Err(());
    };

    use Action as A;

    macro_rules! arg {
        ($errors:expr) => {
            ArgReader {
                action,
                name: &name,
                errors: $errors,
            }
        };
    }

    match normalize_key(&name).as_str() {
        "quit" => Ok(A::Quit(read_bool_arg(
            action,
            "skip_confirmation",
            false,
            errors,
        ))),
        "power_off_monitors" => Ok(A::PowerOffMonitors),
        "power_on_monitors" => Ok(A::PowerOnMonitors),
        "spawn" => {
            let cmd = read_list_of_strings(action, "command", errors);
            Ok(A::Spawn(cmd))
        }
        "spawn_sh" => {
            let cmd = read_string_arg(action, "command", errors).unwrap_or_default();
            Ok(A::SpawnSh(cmd))
        }
        "toggle_debug_tint" => Ok(A::ToggleDebugTint),
        "debug_toggle_opaque_regions" => Ok(A::DebugToggleOpaqueRegions),
        "debug_toggle_damage" => Ok(A::DebugToggleDamage),
        "do_screen_transition" => {
            let mut delay_ms = None;
            match action.get::<Value>("delay_ms") {
                Ok(Value::Integer(i)) if i >= 0 => delay_ms = Some(i as u16),
                Ok(Value::Integer(_)) | Ok(Value::Number(_)) => {
                    errors.push(format!(
                        "action `{name}` requires a non-negative integer `delay_ms`"
                    ));
                }
                Ok(Value::Nil) => {}
                Ok(other) => {
                    errors.push(format!(
                        "action `{name}` requires a non-negative integer `delay_ms`, got {}",
                        type_name(&other)
                    ));
                }
                Err(_) => {}
            }
            Ok(A::DoScreenTransition(delay_ms))
        }
        "close_window" => Ok(A::CloseWindow),
        "fullscreen_window" => Ok(A::FullscreenWindow),
        "toggle_windowed_fullscreen" => Ok(A::ToggleWindowedFullscreen),
        "focus_window_previous" => Ok(A::FocusWindowPrevious),
        "focus_column_left" => Ok(A::FocusColumnLeft),
        "focus_column_right" => Ok(A::FocusColumnRight),
        "focus_column_first" => Ok(A::FocusColumnFirst),
        "focus_column_last" => Ok(A::FocusColumnLast),
        "focus_column_right_or_first" => Ok(A::FocusColumnRightOrFirst),
        "focus_column_left_or_last" => Ok(A::FocusColumnLeftOrLast),
        "focus_column" => arg!(errors).usize("index", A::FocusColumn),
        "focus_window_or_monitor_up" => Ok(A::FocusWindowOrMonitorUp),
        "focus_window_or_monitor_down" => Ok(A::FocusWindowOrMonitorDown),
        "focus_column_or_monitor_left" => Ok(A::FocusColumnOrMonitorLeft),
        "focus_column_or_monitor_right" => Ok(A::FocusColumnOrMonitorRight),
        "focus_window_down" => Ok(A::FocusWindowDown),
        "focus_window_up" => Ok(A::FocusWindowUp),
        "focus_window_down_or_column_left" => Ok(A::FocusWindowDownOrColumnLeft),
        "focus_window_down_or_column_right" => Ok(A::FocusWindowDownOrColumnRight),
        "focus_window_up_or_column_left" => Ok(A::FocusWindowUpOrColumnLeft),
        "focus_window_up_or_column_right" => Ok(A::FocusWindowUpOrColumnRight),
        "focus_window_or_workspace_down" => Ok(A::FocusWindowOrWorkspaceDown),
        "focus_window_or_workspace_up" => Ok(A::FocusWindowOrWorkspaceUp),
        "focus_window_top" => Ok(A::FocusWindowTop),
        "focus_window_bottom" => Ok(A::FocusWindowBottom),
        "focus_window_down_or_top" => Ok(A::FocusWindowDownOrTop),
        "focus_window_up_or_bottom" => Ok(A::FocusWindowUpOrBottom),
        "move_column_left" => Ok(A::MoveColumnLeft),
        "move_column_right" => Ok(A::MoveColumnRight),
        "move_column_to_first" => Ok(A::MoveColumnToFirst),
        "move_column_to_last" => Ok(A::MoveColumnToLast),
        "move_column_to_index" => arg!(errors).usize("index", A::MoveColumnToIndex),
        "move_column_left_or_to_monitor_left" => Ok(A::MoveColumnLeftOrToMonitorLeft),
        "move_column_right_or_to_monitor_right" => Ok(A::MoveColumnRightOrToMonitorRight),
        "move_window_down" => Ok(A::MoveWindowDown),
        "move_window_up" => Ok(A::MoveWindowUp),
        "move_window_left" => Ok(A::MoveWindowLeft),
        "move_window_right" => Ok(A::MoveWindowRight),
        "move_window_down_or_to_workspace_down" => Ok(A::MoveWindowDownOrToWorkspaceDown),
        "move_window_up_or_to_workspace_up" => Ok(A::MoveWindowUpOrToWorkspaceUp),
        "consume_or_expel_window_left" => Ok(A::ConsumeOrExpelWindowLeft),
        "consume_or_expel_window_right" => Ok(A::ConsumeOrExpelWindowRight),
        "consume_window_into_column" => Ok(A::ConsumeWindowIntoColumn),
        "expel_window_from_column" => Ok(A::ExpelWindowFromColumn),
        "toggle_split" => Ok(A::ToggleSplit),
        "preselect" => arg!(errors).enum_parse("direction", SplitDirection::from_str, move |d| {
            A::Preselect(d.into())
        }),
        "promote_window" => Ok(A::PromoteWindow),
        "swap_window_left" => Ok(A::SwapWindowLeft),
        "swap_window_right" => Ok(A::SwapWindowRight),
        "switch_column_display" => Ok(A::SwitchColumnDisplay),
        "set_column_display" => arg!(errors).enum_parse("display", ColumnDisplay::from_str, move |d| {
            A::SetColumnDisplay(d)
        }),
        "center_column" => Ok(A::CenterColumn),
        "center_window" => Ok(A::CenterWindow),
        "center_visible_columns" => Ok(A::CenterVisibleColumns),
        "focus_workspace_down" => Ok(A::FocusWorkspaceDown),
        "focus_workspace_up" => Ok(A::FocusWorkspaceUp),
        "focus_workspace" => {
            let reference = read_workspace_reference(action, errors)?;
            Ok(A::FocusWorkspace(reference))
        }
        "focus_workspace_previous" => Ok(A::FocusWorkspacePrevious),
        "move_window_to_workspace_down" => {
            let focus = read_bool_arg(action, "focus", true, errors);
            Ok(A::MoveWindowToWorkspaceDown(focus))
        }
        "move_window_to_workspace_up" => {
            let focus = read_bool_arg(action, "focus", true, errors);
            Ok(A::MoveWindowToWorkspaceUp(focus))
        }
        "move_window_to_workspace" => {
            let reference = read_workspace_reference(action, errors)?;
            let focus = read_bool_arg(action, "focus", true, errors);
            Ok(A::MoveWindowToWorkspace(reference, focus))
        }
        "move_column_to_workspace_down" => {
            let focus = read_bool_arg(action, "focus", true, errors);
            Ok(A::MoveColumnToWorkspaceDown(focus))
        }
        "move_column_to_workspace_up" => {
            let focus = read_bool_arg(action, "focus", true, errors);
            Ok(A::MoveColumnToWorkspaceUp(focus))
        }
        "move_column_to_workspace" => {
            let reference = read_workspace_reference(action, errors)?;
            let focus = read_bool_arg(action, "focus", true, errors);
            Ok(A::MoveColumnToWorkspace(reference, focus))
        }
        "move_workspace_down" => Ok(A::MoveWorkspaceDown),
        "move_workspace_up" => Ok(A::MoveWorkspaceUp),
        "move_workspace_to_index" => arg!(errors).usize("index", A::MoveWorkspaceToIndex),
        "set_workspace_name" => arg!(errors).string("name", A::SetWorkspaceName),
        "unset_workspace_name" => Ok(A::UnsetWorkspaceName),
        "focus_monitor_left" => Ok(A::FocusMonitorLeft),
        "focus_monitor_right" => Ok(A::FocusMonitorRight),
        "focus_monitor_down" => Ok(A::FocusMonitorDown),
        "focus_monitor_up" => Ok(A::FocusMonitorUp),
        "focus_monitor_previous" => Ok(A::FocusMonitorPrevious),
        "focus_monitor_next" => Ok(A::FocusMonitorNext),
        "focus_monitor" => arg!(errors).string("output", A::FocusMonitor),
        "move_window_to_monitor_left" => Ok(A::MoveWindowToMonitorLeft),
        "move_window_to_monitor_right" => Ok(A::MoveWindowToMonitorRight),
        "move_window_to_monitor_down" => Ok(A::MoveWindowToMonitorDown),
        "move_window_to_monitor_up" => Ok(A::MoveWindowToMonitorUp),
        "move_window_to_monitor_previous" => Ok(A::MoveWindowToMonitorPrevious),
        "move_window_to_monitor_next" => Ok(A::MoveWindowToMonitorNext),
        "move_window_to_monitor" => arg!(errors).string("output", A::MoveWindowToMonitor),
        "move_column_to_monitor_left" => Ok(A::MoveColumnToMonitorLeft),
        "move_column_to_monitor_right" => Ok(A::MoveColumnToMonitorRight),
        "move_column_to_monitor_down" => Ok(A::MoveColumnToMonitorDown),
        "move_column_to_monitor_up" => Ok(A::MoveColumnToMonitorUp),
        "move_column_to_monitor_previous" => Ok(A::MoveColumnToMonitorPrevious),
        "move_column_to_monitor_next" => Ok(A::MoveColumnToMonitorNext),
        "move_column_to_monitor" => arg!(errors).string("output", A::MoveColumnToMonitor),
        "set_window_width" => arg!(errors).size_change("change", A::SetWindowWidth),
        "set_window_height" => arg!(errors).size_change("change", A::SetWindowHeight),
        "reset_window_height" => Ok(A::ResetWindowHeight),
        "switch_preset_column_width" => Ok(A::SwitchPresetColumnWidth),
        "switch_preset_column_width_back" => Ok(A::SwitchPresetColumnWidthBack),
        "switch_preset_window_width" => Ok(A::SwitchPresetWindowWidth),
        "switch_preset_window_width_back" => Ok(A::SwitchPresetWindowWidthBack),
        "switch_preset_window_height" => Ok(A::SwitchPresetWindowHeight),
        "switch_preset_window_height_back" => Ok(A::SwitchPresetWindowHeightBack),
        "maximize_column" => Ok(A::MaximizeColumn),
        "maximize_window_to_edges" => Ok(A::MaximizeWindowToEdges),
        "set_column_width" => arg!(errors).size_change("change", A::SetColumnWidth),
        "expand_column_to_available_width" => Ok(A::ExpandColumnToAvailableWidth),
        "switch_layout" => {
            arg!(errors).enum_parse("layout", LayoutSwitchTarget::from_str, A::SwitchLayout)
        }
        "show_hotkey_overlay" => Ok(A::ShowHotkeyOverlay),
        "move_workspace_to_monitor_left" => Ok(A::MoveWorkspaceToMonitorLeft),
        "move_workspace_to_monitor_right" => Ok(A::MoveWorkspaceToMonitorRight),
        "move_workspace_to_monitor_down" => Ok(A::MoveWorkspaceToMonitorDown),
        "move_workspace_to_monitor_up" => Ok(A::MoveWorkspaceToMonitorUp),
        "move_workspace_to_monitor_previous" => Ok(A::MoveWorkspaceToMonitorPrevious),
        "move_workspace_to_monitor_next" => Ok(A::MoveWorkspaceToMonitorNext),
        "toggle_window_floating" => Ok(A::ToggleWindowFloating),
        "move_window_to_floating" => Ok(A::MoveWindowToFloating),
        "move_window_to_tiling" => Ok(A::MoveWindowToTiling),
        "focus_floating" => Ok(A::FocusFloating),
        "focus_tiling" => Ok(A::FocusTiling),
        "switch_focus_between_floating_and_tiling" => Ok(A::SwitchFocusBetweenFloatingAndTiling),
        "toggle_window_rule_opacity" => Ok(A::ToggleWindowRuleOpacity),
        "toggle_keyboard_shortcuts_inhibit" => Ok(A::ToggleKeyboardShortcutsInhibit),
        "toggle_overview" => Ok(A::ToggleOverview),
        "open_overview" => Ok(A::OpenOverview),
        "close_overview" => Ok(A::CloseOverview),
        "screenshot" => Ok(A::Screenshot(None)),
        "screenshot_screen" => Ok(A::ScreenshotScreen(None)),
        "screenshot_window" => Ok(A::ScreenshotWindow(None)),
        "next_window" => Ok(A::MruAdvance {
            direction: MruDirection::Forward,
            scope: read_mru_scope(action, errors),
            filter: read_mru_filter(action, errors),
        }),
        "previous_window" => Ok(A::MruAdvance {
            direction: MruDirection::Backward,
            scope: read_mru_scope(action, errors),
            filter: read_mru_filter(action, errors),
        }),
        "mru_confirm" => Ok(A::MruConfirm),
        "mru_cancel" => Ok(A::MruCancel),
        "mru_close_current_window" => Ok(A::MruCloseCurrentWindow),
        "mru_first" => Ok(A::MruFirst),
        "mru_last" => Ok(A::MruLast),
        _ => {
            errors.push(format!("unknown action `{name}`"));
            Err(())
        }
    }
}

/// Small helper to read optional typed action arguments.
struct ArgReader<'a, 'b> {
    action: &'a Table,
    name: &'a str,
    errors: &'b mut Vec<String>,
}

impl<'a, 'b> ArgReader<'a, 'b> {
    fn string(self, key: &str, f: impl FnOnce(String) -> Action) -> Result<Action, ()> {
        match self.action.get::<Value>(key) {
            Ok(Value::String(s)) => match s.to_str() {
                Ok(s) => Ok(f(s.to_string())),
                Err(_) => Err(()),
            },
            _ => {
                self.errors.push(format!(
                    "action `{}` requires a `{}` argument (string)",
                    self.name, key
                ));
                Err(())
            }
        }
    }

    fn usize(self, key: &str, f: impl FnOnce(usize) -> Action) -> Result<Action, ()> {
        match self.action.get::<Value>(key) {
            Ok(Value::Integer(i)) if i >= 0 => Ok(f(i as usize)),
            Ok(Value::Integer(_)) | Ok(Value::Number(_)) => {
                self.errors.push(format!(
                    "action `{}` requires a non-negative integer `{}`",
                    self.name, key
                ));
                Err(())
            }
            _ => {
                self.errors.push(format!(
                    "action `{}` requires an integer `{}` argument",
                    self.name, key
                ));
                Err(())
            }
        }
    }

    fn enum_parse<T>(
        self,
        key: &str,
        parse: impl Fn(&str) -> Result<T, <T as FromStr>::Err>,
        f: impl FnOnce(T) -> Action,
    ) -> Result<Action, ()>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        match self.action.get::<Value>(key) {
            Ok(Value::String(s)) => match s.to_str().as_deref() {
                Ok(s) => match parse(s) {
                    Ok(v) => Ok(f(v)),
                    Err(err) => {
                        self.errors.push(format!("action `{}`: {}", self.name, err));
                        Err(())
                    }
                },
                Err(_) => Err(()),
            },
            _ => {
                self.errors.push(format!(
                    "action `{}` requires a `{}` argument",
                    self.name, key
                ));
                Err(())
            }
        }
    }

    fn size_change(self, key: &str, f: impl FnOnce(SizeChange) -> Action) -> Result<Action, ()> {
        match self.action.get::<Value>(key) {
            Ok(Value::String(s)) => match s.to_str().as_deref() {
                Ok(s) => match s.parse::<SizeChange>() {
                    Ok(v) => Ok(f(v)),
                    Err(err) => {
                        self.errors.push(format!("action `{}`: {}", self.name, err));
                        Err(())
                    }
                },
                Err(_) => Err(()),
            },
            _ => {
                self.errors.push(format!(
                    "action `{}` requires a `{}` argument (like \"+10%\" or \"900\")",
                    self.name, key
                ));
                Err(())
            }
        }
    }
}

fn read_bool_arg(action: &Table, key: &str, default: bool, errors: &mut Vec<String>) -> bool {
    match action.get::<Value>(key) {
        Ok(Value::Boolean(b)) => b,
        Ok(Value::Nil) => default,
        Ok(other) => {
            errors.push(format!(
                "`{key}` must be a boolean, got {}",
                type_name(&other)
            ));
            default
        }
        Err(_) => default,
    }
}

fn read_string_arg(action: &Table, key: &str, errors: &mut Vec<String>) -> Option<String> {
    match action.get::<Value>(key) {
        Ok(Value::String(s)) => s.to_str().ok().map(|s| s.to_string()),
        Ok(Value::Nil) => None,
        Ok(other) => {
            errors.push(format!(
                "`{key}` must be a string, got {}",
                type_name(&other)
            ));
            None
        }
        Err(_) => None,
    }
}

fn read_required_string(action: &Table, key: &str, errors: &mut Vec<String>) -> Option<String> {
    match read_string_arg(action, key, errors) {
        Some(s) => Some(s),
        None => {
            errors.push(format!("action requires a `{key}` field"));
            None
        }
    }
}

/// Read an array of strings in index order (Lua tables don't guarantee `pairs` order).
fn read_list_of_strings(action: &Table, key: &str, errors: &mut Vec<String>) -> Vec<String> {
    match action.get::<Value>(key) {
        Ok(Value::Table(t)) => {
            let mut rv = Vec::new();
            for i in 1..=t.raw_len() {
                match t.raw_get::<Value>(i as i64) {
                    Ok(Value::String(s)) => {
                        if let Ok(s) = s.to_str() {
                            rv.push(s.to_string());
                        }
                    }
                    Ok(other) => {
                        errors.push(format!(
                            "`{key}` must be a list of strings, got {} at index {i}",
                            type_name(&other)
                        ));
                        return Vec::new();
                    }
                    Err(_) => break,
                }
            }
            rv
        }
        Ok(Value::Nil) => Vec::new(),
        Ok(other) => {
            errors.push(format!(
                "`{key}` must be a list of strings, got {}",
                type_name(&other)
            ));
            Vec::new()
        }
        Err(_) => Vec::new(),
    }
}

/// Read an array of tables in index order.
fn read_list_of_tables(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<Vec<Table>> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a list, got {}", type_name(value)));
            return None;
        }
    };

    let mut rv = Vec::new();
    for i in 1..=t.raw_len() {
        match t.raw_get::<Value>(i as i64) {
            Ok(Value::Table(item)) => rv.push(item),
            Ok(other) => {
                push_err(
                    errors,
                    key,
                    format!("expected a list of tables, got {} at index {i}", type_name(&other)),
                );
                return None;
            }
            Err(_) => break,
        }
    }

    if rv.is_empty() && t.raw_len() > 0 {
        return None;
    }

    Some(rv)
}

/// Read an array of numbers in index order.
fn read_f32_list(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<Vec<f32>> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a list, got {}", type_name(value)));
            return None;
        }
    };
    let mut rv = Vec::new();
    for i in 1..=t.raw_len() {
        match t.raw_get::<Value>(i as i64) {
            Ok(Value::Integer(n)) => rv.push(n as f32),
            Ok(Value::Number(n)) => rv.push(n as f32),
            Ok(other) => {
                push_err(
                    errors,
                    key,
                    format!("expected a list of numbers, got {}", type_name(&other)),
                );
                return None;
            }
            Err(_) => break,
        }
    }
    Some(rv)
}

fn read_workspace_reference(action: &Table, errors: &mut Vec<String>) -> Result<WorkspaceReference, ()> {
    // Accept `index` (number), `id` (number) or `workspace` (string name).
    match action.get::<Value>("index") {
        Ok(Value::Integer(i)) if (0..=255).contains(&i) => {
            return Ok(WorkspaceReference::Index(i as u8));
        }
        Ok(Value::Integer(_)) | Ok(Value::Number(_)) => {
            errors.push("workspace index must be between 0 and 255".to_string());
            return Err(());
        }
        _ => {}
    }
    match action.get::<Value>("id") {
        Ok(Value::Integer(i)) if i >= 0 => {
            return Ok(WorkspaceReference::Id(i as u64));
        }
        Ok(Value::Integer(_)) | Ok(Value::Number(_)) => {
            errors.push("workspace id must be a non-negative integer".to_string());
            return Err(());
        }
        _ => {}
    }
    if let Ok(Value::String(s)) = action.get::<Value>("workspace") {
        if let Ok(s) = s.to_str() {
            return Ok(WorkspaceReference::Name(s.to_string()));
        }
    }
    errors.push("workspace reference requires `index`, `id` or `workspace`".to_string());
    Err(())
}

fn read_mru_scope(action: &Table, errors: &mut Vec<String>) -> Option<MruScope> {
    match action.get::<Value>("scope") {
        Ok(Value::Nil) => None,
        Ok(Value::String(s)) => match s.to_str().as_deref() {
            Ok("all") => Some(MruScope::All),
            Ok("output") => Some(MruScope::Output),
            Ok("workspace") => Some(MruScope::Workspace),
            _ => {
                errors.push("MRU `scope` must be \"all\", \"output\" or \"workspace\"".to_string());
                None
            }
        },
        Ok(other) => {
            errors.push(format!(
                "MRU `scope` must be a string, got {}",
                type_name(&other)
            ));
            None
        }
        Err(_) => None,
    }
}

fn read_mru_filter(action: &Table, errors: &mut Vec<String>) -> Option<MruFilter> {
    match action.get::<Value>("filter") {
        Ok(Value::Nil) => None,
        Ok(Value::String(s)) => match s.to_str().as_deref() {
            Ok("all") => Some(MruFilter::All),
            Ok("app-id") => Some(MruFilter::AppId),
            _ => {
                errors.push("MRU `filter` must be \"all\" or \"app-id\"".to_string());
                None
            }
        },
        Ok(other) => {
            errors.push(format!(
                "MRU `filter` must be a string, got {}",
                type_name(&other)
            ));
            None
        }
        Err(_) => None,
    }
}

// ===============================================================================================
// Includes
// ===============================================================================================

/// Resolve a path argument: `~` expands to the home dir, anything else resolves relative to the
/// current frame's base directory.
fn resolve_include_path(path: &str, base: &Path) -> PathBuf {
    if let Ok(rest) = Path::new(path).strip_prefix("~") {
        std::env::home_dir()
            .map(|home| home.join(rest))
            .unwrap_or_else(|| PathBuf::from(path))
    } else {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            p
        } else {
            base.join(p)
        }
    }
}

fn make_include_config(lua: &Lua, ctx: Arc<Mutex<ParseCtx>>) -> mlua::Result<Function> {
    let ctx2 = ctx.clone();
    let f = lua.create_function(move |lua, args: MultiValue| {
        let mut args = args.into_iter();
        let path: String = match args.next() {
            Some(Value::String(s)) => s.to_string_lossy(),
            _ => {
                return Err(mlua::Error::RuntimeError(
                    "include_config requires a path".to_string(),
                ))
            }
        };

        let mut optional = false;
        if let Some(Value::Table(opts)) = args.next() {
            if let Value::Boolean(b) = opts_raw_get(&opts, "optional") {
                optional = b;
            }
        }

        let ctx = ctx.clone();

        // Resolve against the current frame's base directory.
        let base = ctx.lock().unwrap().current().base.clone();
        let resolved = resolve_include_path(&path, &base);

        let mut guard = ctx.lock().unwrap();

        // Recursion limit check.
        let recursion = guard.current().recursion + 1;
        if recursion == RECURSION_LIMIT {
            guard.record_include_error(
                &resolved,
                format!(
                    "reached the recursion limit; includes cannot be {RECURSION_LIMIT} levels deep"
                ),
            );
            return Ok(Value::Nil);
        }

        // Recursive-include check.
        if !guard.current_mut().include_stack.insert(resolved.clone()) {
            guard.record_include_error(&resolved, "recursive include (file includes itself)");
            return Ok(Value::Nil);
        }

        // Store the path even if the include fails, so it gets watched.
        guard.includes.push(resolved.clone());

        let Some(text) = (match fs::read_to_string(&resolved) {
            Ok(text) => Some(text),
            Err(err) => {
                if optional && err.kind() == std::io::ErrorKind::NotFound {
                    warn!("optional include not found: {resolved:?}");
                } else {
                    guard.record_include_error(
                        &resolved,
                        format!("failed to read included config from {resolved:?}: {err}"),
                    );
                }
                None
            }
        }) else {
            return Ok(Value::Nil);
        };

        let display_path = relative_to_root(&resolved, &guard.root_base)
            .unwrap_or_else(|| resolved.clone());
        let base = resolved.parent().map(Path::to_path_buf).unwrap_or_default();
        let include_stack = guard.current().include_stack.clone();

        guard.push_frame(Frame {
            base,
            path: display_path.clone(),
            recursion,
            seen: HashSet::new(),
            include_stack,
        });
        drop(guard);

        let result = lua
            .load(&text)
            .set_name(display_path.display().to_string())
            .eval::<Value>();

        let mut guard = ctx.lock().unwrap();
        guard.pop_frame();

        match result {
            Ok(value) => {
                if let Value::Table(table) = &value {
                    drop(guard);
                    apply_data_table(&ctx, table);
                    return Ok(value);
                }
                Ok(value)
            }
            Err(err) => {
                guard.record_include_error(
                    &resolved,
                    format!("failed to parse included config: {}", format_lua_error(&err)),
                );
                Ok(Value::Nil)
            }
        }
    })?;
    let _ = ctx2;
    Ok(f)
}

fn make_dofile(lua: &Lua, ctx: Arc<Mutex<ParseCtx>>) -> mlua::Result<Function> {
    let f = lua.create_function(move |lua, path: String| {
        let base = ctx.lock().unwrap().current().base.clone();
        let resolved = resolve_include_path(&path, &base);
        ctx.lock().unwrap().includes.push(resolved.clone());

        let text = match fs::read_to_string(&resolved) {
            Ok(text) => text,
            Err(err) => {
                return Err(mlua::Error::RuntimeError(format!(
                    "error reading {resolved:?}: {err}"
                )))
            }
        };

        let display_path = relative_to_root(&resolved, &ctx.lock().unwrap().root_base)
            .unwrap_or(resolved);
        lua.load(&text)
            .set_name(display_path.display().to_string())
            .eval::<Value>()
    })?;
    Ok(f)
}

fn make_loadfile(lua: &Lua, ctx: Arc<Mutex<ParseCtx>>) -> mlua::Result<Function> {
    let f = lua.create_function(move |lua, path: String| {
        let base = ctx.lock().unwrap().current().base.clone();
        let resolved = resolve_include_path(&path, &base);
        ctx.lock().unwrap().includes.push(resolved.clone());

        match fs::read_to_string(&resolved) {
            Ok(text) => {
                let display_path =
                    relative_to_root(&resolved, &ctx.lock().unwrap().root_base).unwrap_or(resolved);
                let chunk = lua
                    .load(&text)
                    .set_name(display_path.display().to_string())
                    .into_function()?;
                Ok(MultiValue::from_vec(vec![Value::Function(chunk)]))
            }
            Err(err) => {
                let msg = Value::String(lua.create_string(format!("error reading {resolved:?}: {err}"))?);
                Ok(MultiValue::from_vec(vec![Value::Nil, msg]))
            }
        }
    })?;
    Ok(f)
}

fn make_require(lua: &Lua, ctx: Arc<Mutex<ParseCtx>>) -> mlua::Result<Function> {
    let globals = lua.globals();
    let package: Table = globals.get("package")?;

    let f = lua.create_function(move |lua, name: String| {
        let loaded: Table = lua.globals().get::<Table>("package")?.get("loaded")?;
        match loaded.raw_get::<Value>(name.clone()) {
            Ok(value) if !matches!(value, Value::Nil) => return Ok(value),
            _ => {}
        }

        let base = ctx.lock().unwrap().current().base.clone();
        // Convert "a.b.c" to "a/b/c.lua".
        let rel = name.replace('.', "/");
        let mut candidates = vec![base.join(format!("{rel}.lua"))];
        if let Some(home) = std::env::home_dir() {
            candidates.push(home.join(".config/ymir").join(format!("{rel}.lua")));
        }

        let mut text: Option<String> = None;
        let mut resolved: Option<PathBuf> = None;
        for candidate in candidates {
            if let Ok(x) = fs::read_to_string(&candidate) {
                text = Some(x);
                resolved = Some(candidate);
                break;
            }
        }

        let (Some(text), Some(resolved)) = (text, resolved) else {
            return Err(mlua::Error::RuntimeError(format!(
                "module '{name}' not found"
            )));
        };

        ctx.lock().unwrap().includes.push(resolved.clone());
        let display_path = relative_to_root(&resolved, &ctx.lock().unwrap().root_base)
            .unwrap_or(resolved);

        let result = lua
            .load(&text)
            .set_name(display_path.display().to_string())
            .eval::<Value>()?;

        let _ = &package;
        if !matches!(result, Value::Nil) {
            loaded.raw_set(name, result.clone())?;
        }
        Ok(result)
    })?;

    Ok(f)
}

// ===============================================================================================
// Shared table readers
// ===============================================================================================

/// Borrow an "optional" table field as a `Value`.
fn opts_raw_get(t: &Table, key: &str) -> Value {
    t.get::<Value>(key).unwrap_or(Value::Nil)
}

fn read_flag_default(value: &Value, default: bool) -> bool {
    match value {
        Value::Nil => default,
        Value::Boolean(b) => *b,
        _ => default,
    }
}

/// Read a `Flag`-style value: any present value counts as enabled unless it is a boolean.
///
/// Mirrors a tri-state: `field` -> on, `field true`/`field false` -> explicit.
fn read_flag_value(value: &Value) -> Option<Flag> {
    match value {
        Value::Boolean(b) => Some(Flag(*b)),
        _ => Some(Flag(true)),
    }
}

/// Read a `Flag`-style boolean (only actually set when the value is `true`).
fn read_flag(value: &Value) -> bool {
    matches!(value, Value::Boolean(true))
}

fn read_bool_or_flag(value: &Value) -> Option<bool> {
    match value {
        Value::Boolean(b) => Some(*b),
        _ => None,
    }
}

fn read_duration_ms(t: &Table, key: &str, errors: &mut Vec<String>) -> Option<std::time::Duration> {
    match t.get::<Value>(key) {
        Ok(Value::Integer(i)) if i > 0 => Some(std::time::Duration::from_millis(i as u64)),
        Ok(Value::Nil) => None,
        Ok(other) => {
            errors.push(format!(
                "`{key}` must be a positive integer (milliseconds), got {}",
                type_name(&other)
            ));
            None
        }
        Err(_) => None,
    }
}

fn read_hotkey_overlay_title(t: &Table, errors: &mut Vec<String>) -> Option<Option<String>> {
    match t.get::<Value>("hotkey_overlay_title") {
        Ok(Value::Nil) => None,
        Ok(Value::Boolean(false)) => Some(None),
        Ok(Value::LightUserData(u)) if u.0.is_null() => Some(None),
        Ok(Value::String(s)) => Some(Some(s.to_string_lossy())),
        Ok(other) => {
            errors.push(format!(
                "`hotkey_overlay_title` must be a string or `false`, got {}",
                type_name(&other)
            ));
            None
        }
        Err(_) => None,
    }
}

fn read_float_or_int<const MIN: i32, const MAX: i32>(
    value: &Value,
    key: &str,
    errors: &mut Vec<String>,
) -> Option<FloatOrInt<MIN, MAX>> {
    let f = match value {
        Value::Integer(i) => *i as f64,
        Value::Number(n) => *n,
        _ => {
            errors.push(format!("`{key}` must be a number, got {}", type_name(value)));
            return None;
        }
    };
    if f < MIN as f64 || f > MAX as f64 {
        errors.push(format!("`{key}` must be between {MIN} and {MAX}, got {f}"));
        return None;
    }
    Some(FloatOrInt(f))
}

fn read_f64(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<f64> {
    match value {
        Value::Integer(i) => Some(*i as f64),
        Value::Number(n) => Some(*n),
        _ => {
            errors.push(format!("`{key}` must be a number, got {}", type_name(value)));
            None
        }
    }
}

fn read_u16(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<u16> {
    match value {
        Value::Integer(i) if (0..=u16::MAX as i64).contains(i) => Some(*i as u16),
        Value::Integer(_) => {
            errors.push(format!("`{key}` must be between 0 and 65535"));
            None
        }
        _ => {
            errors.push(format!("`{key}` must be a number, got {}", type_name(value)));
            None
        }
    }
}

fn read_u8(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<u8> {
    match value {
        Value::Integer(i) if (0..=u8::MAX as i64).contains(i) => Some(*i as u8),
        Value::Integer(_) => {
            errors.push(format!("`{key}` must be between 0 and 255"));
            None
        }
        _ => {
            errors.push(format!("`{key}` must be a number, got {}", type_name(value)));
            None
        }
    }
}

fn read_u32(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<u32> {
    match value {
        Value::Integer(i) if (0..=u32::MAX as i64).contains(i) => Some(*i as u32),
        Value::Integer(_) => {
            errors.push(format!("`{key}` must be a non-negative number"));
            None
        }
        _ => {
            errors.push(format!("`{key}` must be a number, got {}", type_name(value)));
            None
        }
    }
}

fn read_i32(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<i32> {
    match value {
        Value::Integer(i) if (i32::MIN as i64..=i32::MAX as i64).contains(i) => Some(*i as i32),
        Value::Integer(_) => {
            errors.push(format!("`{key}` must fit in a 32-bit integer"));
            None
        }
        _ => {
            errors.push(format!("`{key}` must be a number, got {}", type_name(value)));
            None
        }
    }
}

/// A positive integer (>= 1) field.
fn read_positive_usize(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<usize> {
    match value {
        Value::Integer(i) if *i >= 1 => Some(*i as usize),
        Value::Integer(_) => {
            errors.push(format!("`{key}` must be a positive number"));
            None
        }
        _ => {
            errors.push(format!("`{key}` must be a number, got {}", type_name(value)));
            None
        }
    }
}

/// An enum field parsed via `FromStr`.
fn read_enum<T>(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<T>
where
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    match value {
        Value::String(s) => match s.to_str().map(|s| s.parse()) {
            Ok(Ok(v)) => Some(v),
            _ => {
                errors.push(format!("`{key}`: invalid value `{}`", s.to_string_lossy()));
                None
            }
        },
        _ => {
            errors.push(format!("`{key}` must be a string, got {}", type_name(value)));
            None
        }
    }
}

/// A string field; used for names, output names, etc.
fn read_string(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<String> {
    match value {
        Value::String(s) => s.to_str().ok().map(|s| s.to_string()),
        _ => {
            errors.push(format!("`{key}` must be a string, got {}", type_name(value)));
            None
        }
    }
}

/// Read a color: CSS string or `{ r, g, b, a }` table (0-255).
fn read_color(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<Color> {
    match value {
        Value::String(s) => match Color::from_str(&lua_str(s)) {
            Ok(color) => Some(color),
            Err(err) => {
                errors.push(format!("`{key}`: {err}"));
                None
            }
        },
        Value::Table(t) => {
            let mut rgba = [0u8, 0, 0, 255];
            let mut valid = true;
            for (name, idx) in [("r", 0usize), ("g", 1), ("b", 2), ("a", 3)] {
                match opts_raw_get(t, name) {
                    Value::Integer(i) if (0..=255).contains(&i) => rgba[idx] = i as u8,
                    Value::Number(n) if (0.0..=255.0).contains(&n) => rgba[idx] = n.round() as u8,
                    Value::Nil if idx < 3 => {
                        errors.push(format!("`{key}` requires `{name}`"));
                        valid = false;
                    }
                    other => {
                        errors.push(format!(
                            "`{key}`.{name} must be a number between 0 and 255, got {}",
                            type_name(&other)
                        ));
                        valid = false;
                    }
                }
            }
            if valid {
                Some(Color::from_rgba8_unpremul(rgba[0], rgba[1], rgba[2], rgba[3]))
            } else {
                None
            }
        }
        _ => {
            errors.push(format!(
                "`{key}` must be a color string or table, got {}",
                type_name(value)
            ));
            None
        }
    }
}

/// Read a gradient table `{ from, to, angle?, relative_to?, in? }`.
///
/// Defaults: `angle` 180, `relative_to` window, `in` `GradientInterpolation::default()`.
fn read_gradient(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<Gradient> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            errors.push(format!("`{key}` must be a table, got {}", type_name(value)));
            return None;
        }
    };

    let from = read_color(&opts_raw_get(t, "from").clone(), &format!("{key}.from"), errors)?;
    let to = read_color(&opts_raw_get(t, "to").clone(), &format!("{key}.to"), errors)?;

    let angle = match opts_raw_get(t, "angle") {
        Value::Nil => 180,
        other => read_f64(&other, &format!("{key}.angle"), errors).unwrap_or_default() as i16,
    };

    let relative_to = match opts_raw_get(t, "relative_to") {
        Value::Nil => GradientRelativeTo::Window,
        Value::String(s) => match s.to_str().as_deref() {
            Ok("window") => GradientRelativeTo::Window,
            Ok("workspace-view") => GradientRelativeTo::WorkspaceView,
            _ => {
                errors.push(format!("`{key}.relative_to` must be \"window\" or \"workspace-view\""));
                GradientRelativeTo::Window
            }
        },
        other => {
            errors.push(format!(
                "`{key}.relative_to` must be a string, got {}",
                type_name(&other)
            ));
            GradientRelativeTo::Window
        }
    };

    let in_ = match opts_raw_get(t, "in") {
        Value::Nil => GradientInterpolation::default(),
        Value::String(s) => match s.to_str().map(|s| s.parse()) {
            Ok(Ok(v)) => v,
            _ => {
                errors.push(format!("`{key}.in`: invalid interpolation"));
                GradientInterpolation::default()
            }
        },
        other => {
            errors.push(format!(
                "`{key}.in` must be a string, got {}",
                type_name(&other)
            ));
            GradientInterpolation::default()
        }
    };

    Some(Gradient {
        from,
        to,
        angle,
        relative_to,
        in_,
    })
}

/// Read a corner radius: a number applies to all corners, a table applies per-corner.
fn read_corner_radius(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<CornerRadius> {
    match value {
        Value::Integer(i) => Some(CornerRadius::from(*i as f32)),
        Value::Number(n) => Some(CornerRadius::from(*n as f32)),
        Value::Table(t) => {
            let mut radius = CornerRadius::default();
            let mut any = false;
            for (name, field) in [
                ("top_left", &mut radius.top_left as *mut f32),
                ("top_right", &mut radius.top_right as *mut f32),
                ("bottom_left", &mut radius.bottom_left as *mut f32),
                ("bottom_right", &mut radius.bottom_right as *mut f32),
            ] {
                unsafe {
                    match opts_raw_get(t, name) {
                        Value::Integer(i) => {
                            *field = i as f32;
                            any = true;
                        }
                        Value::Number(n) => {
                            *field = n as f32;
                            any = true;
                        }
                        Value::Nil => {}
                        other => {
                            errors.push(format!(
                                "`{key}`.{name} must be a number, got {}",
                                type_name(&other)
                            ));
                        }
                    }
                }
            }
            if any {
                Some(radius)
            } else {
                errors.push(format!("`{key}` must have at least one corner"));
                None
            }
        }
        _ => {
            errors.push(format!(
                "`{key}` must be a number or a table, got {}",
                type_name(value)
            ));
            None
        }
    }
}

/// Read a shadow offset table `{ x, y }`.
fn read_shadow_offset(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<ShadowOffset> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };

    let mut offset = ShadowOffset {
        x: FloatOrInt(0.),
        y: FloatOrInt(0.),
    };
    for (name, field) in [("x", &mut offset.x as *mut FloatOrInt<-65535, 65535>), ("y", &mut offset.y as *mut FloatOrInt<-65535, 65535>)] {
        unsafe {
            match opts_raw_get(t, name) {
                Value::Integer(i) => {
                    *field = FloatOrInt(i as f64);
                }
                Value::Number(n) => {
                    *field = FloatOrInt(n);
                }
                Value::Nil => {}
                other => {
                    errors.push(format!(
                        "`{key}`.{name} must be a number, got {}",
                        type_name(&other)
                    ));
                }
            }
        }
    }
    Some(offset)
}

fn push_err(errors: &mut Vec<String>, key: &str, msg: impl std::fmt::Display) {
    errors.push(format!("{key}: {msg}"));
}

// ===============================================================================================
// Section appliers
// ===============================================================================================

fn apply_input(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return;
        }
    };

    let mut part = InputPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "keyboard" => {
                if let Some(kb) = read_keyboard(&value, errors) {
                    part.keyboard = Some(kb);
                }
            }
            "touchpad" => {
                if let Some(p) = read_pointer(&value, "input.touchpad", errors, PointerKind::Touchpad) {
                    part.touchpad = Some(p.into_touchpad());
                }
            }
            "mouse" => {
                if let Some(p) = read_pointer(&value, "input.mouse", errors, PointerKind::Mouse) {
                    part.mouse = Some(p.into_mouse());
                }
            }
            "trackpoint" => {
                if let Some(p) = read_pointer(&value, "input.trackpoint", errors, PointerKind::Trackpoint) {
                    part.trackpoint = Some(p.into_trackpoint());
                }
            }
            "trackball" => {
                if let Some(p) = read_pointer(&value, "input.trackball", errors, PointerKind::Trackball) {
                    part.trackball = Some(p.into_trackball());
                }
            }
            "tablet" => {
                if let Some(v) = read_tablet(&value, errors) {
                    part.tablet = Some(v);
                }
            }
            "touch" => {
                if let Some(v) = read_touch(&value, errors) {
                    part.touch = Some(v);
                }
            }
            "disable_power_key_handling" => part.disable_power_key_handling = read_flag_value(&value),
            "warp_mouse_to_focus" => part.warp_mouse_to_focus = read_warp_mouse_to_focus(&value, errors),
            "focus_follows_mouse" => part.focus_follows_mouse = read_focus_follows_mouse(&value, errors),
            "workspace_auto_back_and_forth" => part.workspace_auto_back_and_forth = read_flag_value(&value),
            "mod_key" => part.mod_key = read_enum(&value, "input.mod_key", errors),
            "mod_key_nested" => part.mod_key_nested = read_enum(&value, "input.mod_key_nested", errors),
            _ => push_err(errors, "input", format!("unexpected key `{key}`")),
        }
    }

    config.input.merge_with(&part);
}

fn read_keyboard(value: &Value, errors: &mut Vec<String>) -> Option<KeyboardPart> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, "input.keyboard", format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };

    let mut kb = KeyboardPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "xkb" => kb.xkb = read_xkb(&value, errors),
            "repeat_delay" => kb.repeat_delay = read_u16(&value, "input.keyboard.repeat_delay", errors),
            "repeat_rate" => kb.repeat_rate = read_u8(&value, "input.keyboard.repeat_rate", errors),
            "track_layout" => {
                kb.track_layout = match value {
                    Value::String(s) => match s.to_str().as_deref() {
                        Ok("global") => Some(TrackLayout::Global),
                        Ok("window") => Some(TrackLayout::Window),
                        _ => {
                            push_err(
                                errors,
                                "input.keyboard.track_layout",
                                format!("invalid value `{}`", s.to_string_lossy()),
                            );
                            None
                        }
                    },
                    other => {
                        push_err(
                            errors,
                            "input.keyboard.track_layout",
                            format!("expected a string, got {}", type_name(&other)),
                        );
                        None
                    }
                };
            }
            "numlock" => kb.numlock = read_flag_value(&value),
            _ => push_err(errors, "input.keyboard", format!("unexpected key `{key}`")),
        }
    }
    if kb.xkb.is_none()
        && kb.repeat_delay.is_none()
        && kb.repeat_rate.is_none()
        && kb.track_layout.is_none()
        && kb.numlock.is_none()
    {
        return None;
    }
    Some(kb)
}

fn read_xkb(value: &Value, errors: &mut Vec<String>) -> Option<Xkb> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, "input.keyboard.xkb", format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };
    let mut xkb = Xkb::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "rules" => xkb.rules = read_string(&value, "input.keyboard.xkb.rules", errors).unwrap_or_default(),
            "model" => xkb.model = read_string(&value, "input.keyboard.xkb.model", errors).unwrap_or_default(),
            "layout" => xkb.layout = read_string(&value, "input.keyboard.xkb.layout", errors).unwrap_or_default(),
            "variant" => xkb.variant = read_string(&value, "input.keyboard.xkb.variant", errors).unwrap_or_default(),
            "options" => xkb.options = read_string(&value, "input.keyboard.xkb.options", errors),
            "file" => xkb.file = read_string(&value, "input.keyboard.xkb.file", errors),
            _ => push_err(errors, "input.keyboard.xkb", format!("unexpected key `{key}`")),
        }
    }
    Some(xkb)
}

#[derive(Clone, Copy)]
enum PointerKind {
    Touchpad,
    Mouse,
    Trackpoint,
    Trackball,
}

/// Raw pointer fields shared between the touchpad/mouse/trackpoint/trackball variants.
#[derive(Default)]
struct RawPointer {
    off: bool,
    tap: Option<Flag>,
    dwt: Option<Flag>,
    dwtp: Option<Flag>,
    drag: Option<bool>,
    drag_lock: Option<Flag>,
    natural_scroll: Option<Flag>,
    click_method: Option<ClickMethod>,
    accel_speed: Option<FloatOrInt<-1, 1>>,
    accel_profile: Option<AccelProfile>,
    scroll_method: Option<ScrollMethod>,
    scroll_button: Option<u32>,
    scroll_button_lock: Option<Flag>,
    tap_button_map: Option<TapButtonMap>,
    left_handed: Option<Flag>,
    disabled_on_external_mouse: Option<Flag>,
    middle_emulation: Option<Flag>,
    scroll_factor: Option<ScrollFactor>,
}

impl RawPointer {
    fn into_touchpad(self) -> Touchpad {
        Touchpad {
            off: self.off,
            tap: self.tap.is_some_and(|f| f.0),
            dwt: self.dwt.is_some_and(|f| f.0),
            dwtp: self.dwtp.is_some_and(|f| f.0),
            drag: self.drag,
            drag_lock: self.drag_lock.is_some_and(|f| f.0),
            natural_scroll: self.natural_scroll.is_some_and(|f| f.0),
            click_method: self.click_method,
            accel_speed: self.accel_speed.unwrap_or_default(),
            accel_profile: self.accel_profile,
            scroll_method: self.scroll_method,
            scroll_button: self.scroll_button,
            scroll_button_lock: self.scroll_button_lock.is_some_and(|f| f.0),
            tap_button_map: self.tap_button_map,
            left_handed: self.left_handed.is_some_and(|f| f.0),
            disabled_on_external_mouse: self
                .disabled_on_external_mouse
                .is_some_and(|f| f.0),
            middle_emulation: self.middle_emulation.is_some_and(|f| f.0),
            scroll_factor: self.scroll_factor,
        }
    }

    fn into_mouse(self) -> Mouse {
        Mouse {
            off: self.off,
            natural_scroll: self.natural_scroll.is_some_and(|f| f.0),
            accel_speed: self.accel_speed.unwrap_or_default(),
            accel_profile: self.accel_profile,
            scroll_method: self.scroll_method,
            scroll_button: self.scroll_button,
            scroll_button_lock: self.scroll_button_lock.is_some_and(|f| f.0),
            left_handed: self.left_handed.is_some_and(|f| f.0),
            middle_emulation: self.middle_emulation.is_some_and(|f| f.0),
            scroll_factor: self.scroll_factor,
        }
    }

    fn into_trackpoint(self) -> Trackpoint {
        Trackpoint {
            off: self.off,
            natural_scroll: self.natural_scroll.is_some_and(|f| f.0),
            accel_speed: self.accel_speed.unwrap_or_default(),
            accel_profile: self.accel_profile,
            scroll_method: self.scroll_method,
            scroll_button: self.scroll_button,
            scroll_button_lock: self.scroll_button_lock.is_some_and(|f| f.0),
            left_handed: self.left_handed.is_some_and(|f| f.0),
            middle_emulation: self.middle_emulation.is_some_and(|f| f.0),
        }
    }

    fn into_trackball(self) -> Trackball {
        Trackball {
            off: self.off,
            natural_scroll: self.natural_scroll.is_some_and(|f| f.0),
            accel_speed: self.accel_speed.unwrap_or_default(),
            accel_profile: self.accel_profile,
            scroll_method: self.scroll_method,
            scroll_button: self.scroll_button,
            scroll_button_lock: self.scroll_button_lock.is_some_and(|f| f.0),
            left_handed: self.left_handed.is_some_and(|f| f.0),
            middle_emulation: self.middle_emulation.is_some_and(|f| f.0),
        }
    }
}

fn read_pointer(
    value: &Value,
    prefix: &str,
    errors: &mut Vec<String>,
    kind: PointerKind,
) -> Option<RawPointer> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, prefix, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };

    let mut p = RawPointer::default();
    let mut any = false;
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        let touchpad_only = matches!(
            key.as_str(),
            "tap" | "dwt" | "dwtp" | "drag" | "drag_lock" | "click_method" | "tap_button_map" | "disabled_on_external_mouse"
        );
        if touchpad_only && !matches!(kind, PointerKind::Touchpad) {
            push_err(errors, prefix, format!("unexpected key `{key}`"));
            continue;
        }
        if matches!(key.as_str(), "scroll_factor") && !matches!(kind, PointerKind::Touchpad | PointerKind::Mouse) {
            push_err(errors, prefix, format!("unexpected key `{key}`"));
            continue;
        }

        match key.as_str() {
            "off" => {
                p.off = read_flag(&value);
                any = true;
            }
            "tap" => {
                p.tap = read_flag_value(&value);
                any = true;
            }
            "dwt" => {
                p.dwt = read_flag_value(&value);
                any = true;
            }
            "dwtp" => {
                p.dwtp = read_flag_value(&value);
                any = true;
            }
            "drag" => {
                p.drag = read_bool_or_flag(&value);
                any = true;
            }
            "drag_lock" => {
                p.drag_lock = read_flag_value(&value);
                any = true;
            }
            "natural_scroll" => {
                p.natural_scroll = read_flag_value(&value);
                any = true;
            }
            "click_method" => {
                p.click_method = read_enum(&value, &format!("{prefix}.click_method"), errors);
                any = true;
            }
            "accel_speed" => {
                p.accel_speed = read_float_or_int(&value, &format!("{prefix}.accel_speed"), errors);
                any = true;
            }
            "accel_profile" => {
                p.accel_profile = read_enum(&value, &format!("{prefix}.accel_profile"), errors);
                any = true;
            }
            "scroll_method" => {
                p.scroll_method = read_enum(&value, &format!("{prefix}.scroll_method"), errors);
                any = true;
            }
            "scroll_button" => {
                p.scroll_button = read_u32(&value, &format!("{prefix}.scroll_button"), errors);
                any = true;
            }
            "scroll_button_lock" => {
                p.scroll_button_lock = read_flag_value(&value);
                any = true;
            }
            "tap_button_map" => {
                p.tap_button_map = read_enum(&value, &format!("{prefix}.tap_button_map"), errors);
                any = true;
            }
            "left_handed" => {
                p.left_handed = read_flag_value(&value);
                any = true;
            }
            "disabled_on_external_mouse" => {
                p.disabled_on_external_mouse = read_flag_value(&value);
                any = true;
            }
            "middle_emulation" => {
                p.middle_emulation = read_flag_value(&value);
                any = true;
            }
            "scroll_factor" => {
                p.scroll_factor = read_scroll_factor(&value, &format!("{prefix}.scroll_factor"), errors);
                any = true;
            }
            _ => push_err(errors, prefix, format!("unexpected key `{key}`")),
        }
    }
    any.then_some(p)
}

fn read_scroll_factor(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<ScrollFactor> {
    match value {
        Value::Number(n) => Some(ScrollFactor {
            base: Some(FloatOrInt(*n)),
            horizontal: None,
            vertical: None,
        }),
        Value::Integer(i) => Some(ScrollFactor {
            base: Some(FloatOrInt(*i as f64)),
            horizontal: None,
            vertical: None,
        }),
        Value::Table(t) => {
            let mut sf = ScrollFactor::default();
            let it = t.pairs::<String, Value>();
            for kv in it {
                let (k, v) = match kv { Ok(kv) => kv, Err(_) => continue };
                let k = normalize_key(&k);
                match k.as_str() {
                    "base" => sf.base = read_float_or_int(&v, &format!("{key}.base"), errors),
                    "horizontal" => sf.horizontal = read_float_or_int(&v, &format!("{key}.horizontal"), errors),
                    "vertical" => sf.vertical = read_float_or_int(&v, &format!("{key}.vertical"), errors),
                    _ => push_err(errors, key, format!("unexpected key `{k}`")),
                }
            }
            Some(sf)
        }
        _ => {
            push_err(errors, key, format!("must be a number or a table, got {}", type_name(value)));
            None
        }
    }
}

fn read_tablet(value: &Value, errors: &mut Vec<String>) -> Option<Tablet> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, "input.tablet", format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };
    let mut tablet = Tablet::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "off" => tablet.off = read_flag(&value),
            "calibration_matrix" => tablet.calibration_matrix = read_f32_list(&value, "input.tablet.calibration_matrix", errors),
            "map_to_output" => tablet.map_to_output = read_string(&value, "input.tablet.map_to_output", errors),
            "map_to_focused_output" => tablet.map_to_focused_output = read_flag(&value),
            "map_to_focused_window" => tablet.map_to_focused_window = read_flag(&value),
            "left_handed" => tablet.left_handed = read_flag(&value),
            _ => push_err(errors, "input.tablet", format!("unexpected key `{key}`")),
        }
    }
    Some(tablet)
}

fn read_touch(value: &Value, errors: &mut Vec<String>) -> Option<Touch> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, "input.touch", format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };
    let mut touch = Touch::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "off" => touch.off = read_flag(&value),
            "calibration_matrix" => touch.calibration_matrix = read_f32_list(&value, "input.touch.calibration_matrix", errors),
            "map_to_output" => touch.map_to_output = read_string(&value, "input.touch.map_to_output", errors),
            _ => push_err(errors, "input.touch", format!("unexpected key `{key}`")),
        }
    }
    Some(touch)
}

fn read_warp_mouse_to_focus(value: &Value, errors: &mut Vec<String>) -> Option<WarpMouseToFocus> {
    match value {
        Value::Boolean(_) => Some(WarpMouseToFocus { mode: None }),
        Value::Nil => None,
        Value::String(s) => {
            let mode = s.to_str().ok()?.parse::<WarpMouseToFocusMode>().ok()?;
            Some(WarpMouseToFocus { mode: Some(mode) })
        }
        Value::Table(t) => {
            let mode = match opts_raw_get(t, "mode") {
                Value::Nil => None,
                Value::String(s) => match s.to_str().map(|s| s.parse()) {
                    Ok(Ok(m)) => Some(m),
                    _ => {
                        push_err(errors, "input.warp_mouse_to_focus.mode", "invalid value");
                        None
                    }
                },
                _ => {
                    push_err(errors, "input.warp_mouse_to_focus.mode", "must be a string");
                    None
                }
            };
            Some(WarpMouseToFocus { mode })
        }
        _ => {
            push_err(errors, "warp_mouse_to_focus", "invalid value");
            None
        }
    }
}

fn read_focus_follows_mouse(value: &Value, errors: &mut Vec<String>) -> Option<FocusFollowsMouse> {
    match value {
        Value::Boolean(_) => Some(FocusFollowsMouse {
            max_scroll_amount: None,
        }),
        Value::Nil => None,
        Value::String(s) => {
            let percent = s.to_str().ok()?.parse::<Percent>().ok()?;
            Some(FocusFollowsMouse {
                max_scroll_amount: Some(percent),
            })
        }
        Value::Table(t) => {
            let max = read_percent(
                &opts_raw_get(t, "max_scroll_amount"),
                "input.focus_follows_mouse.max_scroll_amount",
                errors,
            );
            Some(FocusFollowsMouse {
                max_scroll_amount: max,
            })
        }
        _ => {
            push_err(errors, "focus_follows_mouse", "invalid value");
            None
        }
    }
}

fn read_percent(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<Percent> {
    match value {
        Value::Number(n) => Some(Percent(*n)),
        Value::Integer(i) => Some(Percent(*i as f64)),
        Value::String(s) => match s.to_str().as_deref() {
            Ok(s) => {
                match s.parse::<Percent>() {
                    Ok(p) => Some(p),
                    Err(err) => {
                        errors.push(format!("`{key}`: {err}"));
                        None
                    }
                }
            }
            Err(_) => None,
        },
        _ => {
            errors.push(format!(
                "`{key}` must be a number or a percentage string, got {}",
                type_name(value)
            ));
            None
        }
    }
}

fn apply_cursor(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return;
        }
    };

    let mut part = CursorPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "xcursor_theme" => part.xcursor_theme = read_string(&value, "cursor.xcursor_theme", errors),
            "xcursor_size" => part.xcursor_size = read_u8(&value, "cursor.xcursor_size", errors),
            "hide_when_typing" => part.hide_when_typing = read_flag_value(&value),
            "hide_after_inactive_ms" => part.hide_after_inactive_ms = read_u32(&value, "cursor.hide_after_inactive_ms", errors),
            _ => push_err(errors, "cursor", format!("unexpected key `{key}`")),
        }
    }

    config.cursor.merge_with(&part);
}

fn apply_clipboard(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return;
        }
    };

    let mut part = ClipboardPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "disable_primary" => part.disable_primary = read_flag_value(&value),
            _ => push_err(errors, "clipboard", format!("unexpected key `{key}`")),
        }
    }

    config.clipboard.merge_with(&part);
}

fn apply_hotkey_overlay(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return;
        }
    };

    let mut part = HotkeyOverlayPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "skip_at_startup" => part.skip_at_startup = read_flag_value(&value),
            "hide_not_bound" => part.hide_not_bound = read_flag_value(&value),
            _ => push_err(errors, "hotkey_overlay", format!("unexpected key `{key}`")),
        }
    }

    config.hotkey_overlay.merge_with(&part);
}

fn apply_config_notification(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return;
        }
    };

    let mut part = ConfigNotificationPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "disable_failed" => part.disable_failed = read_flag_value(&value),
            _ => push_err(errors, "config_notification", format!("unexpected key `{key}`")),
        }
    }

    config.config_notification.merge_with(&part);
}

fn apply_animations(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return;
        }
    };

    let mut part = AnimationsPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "off" => part.off = read_flag(&value),
            "on" => part.on = read_flag(&value),
            "slowdown" => part.slowdown = read_float_or_int(&value, "animations.slowdown", errors),
            "workspace_switch" => {
                if let Some(anim) = read_animation(&value, "animations.workspace_switch", errors) {
                    part.workspace_switch = Some(WorkspaceSwitchAnim(anim));
                }
            }
            "window_open" => {
                if let Some(anim) = read_window_open_anim(&value, "animations.window_open", errors) {
                    part.window_open = Some(anim);
                }
            }
            "window_close" => {
                if let Some(anim) = read_window_close_anim(&value, "animations.window_close", errors) {
                    part.window_close = Some(anim);
                }
            }
            "horizontal_view_movement" => {
                if let Some(anim) = read_animation(&value, "animations.horizontal_view_movement", errors) {
                    part.horizontal_view_movement = Some(HorizontalViewMovementAnim(anim));
                }
            }
            "window_movement" => {
                if let Some(anim) = read_animation(&value, "animations.window_movement", errors) {
                    part.window_movement = Some(WindowMovementAnim(anim));
                }
            }
            "window_resize" => {
                if let Some(anim) = read_window_resize_anim(&value, "animations.window_resize", errors) {
                    part.window_resize = Some(anim);
                }
            }
            "config_notification_open_close" => {
                if let Some(anim) = read_animation(&value, "animations.config_notification_open_close", errors) {
                    part.config_notification_open_close = Some(ConfigNotificationOpenCloseAnim(anim));
                }
            }
            "exit_confirmation_open_close" => {
                if let Some(anim) = read_animation(&value, "animations.exit_confirmation_open_close", errors) {
                    part.exit_confirmation_open_close = Some(ExitConfirmationOpenCloseAnim(anim));
                }
            }
            "screenshot_ui_open" => {
                if let Some(anim) = read_animation(&value, "animations.screenshot_ui_open", errors) {
                    part.screenshot_ui_open = Some(ScreenshotUiOpenAnim(anim));
                }
            }
            "overview_open_close" => {
                if let Some(anim) = read_animation(&value, "animations.overview_open_close", errors) {
                    part.overview_open_close = Some(OverviewOpenCloseAnim(anim));
                }
            }
            "recent_windows_close" => {
                if let Some(anim) = read_animation(&value, "animations.recent_windows_close", errors) {
                    part.recent_windows_close = Some(RecentWindowsCloseAnim(anim));
                }
            }
            _ => push_err(errors, "animations", format!("unexpected key `{key}`")),
        }
    }

    config.animations.merge_with(&part);
}

fn read_animation(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<Animation> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };

    let off = read_flag(&opts_raw_get(t, "off"));

    let kind = match opts_raw_get(t, "spring") {
        Value::Table(spring) => {
            let damping_ratio = match opts_raw_get(&spring, "damping_ratio") {
                Value::Nil => 1.0,
                other => read_f64(&other, &format!("{key}.spring.damping_ratio"), errors).unwrap_or(1.0),
            };
            let epsilon = match opts_raw_get(&spring, "epsilon") {
                Value::Nil => 0.0001,
                other => read_f64(&other, &format!("{key}.spring.epsilon"), errors).unwrap_or(0.0001),
            };
            let stiffness = match opts_raw_get(&spring, "stiffness") {
                Value::Integer(i) if i >= 0 => i as u32,
                Value::Integer(_) => {
                    push_err(errors, key, "spring stiffness must be a non-negative integer");
                    return None;
                }
                other => {
                    push_err(
                        errors,
                        &format!("{key}.spring.stiffness"),
                        format!("expected an integer, got {}", type_name(&other)),
                    );
                    return None;
                }
            };
            Some(Kind::Spring(SpringParams {
                damping_ratio,
                stiffness,
                epsilon,
            }))
        }
        Value::Nil => match opts_raw_get(t, "easing") {
            Value::Table(easing) => {
                let duration_ms = match opts_raw_get(&easing, "duration_ms") {
                    Value::Integer(i) if i >= 0 => i as u32,
                    other => {
                        push_err(
                            errors,
                            &format!("{key}.easing.duration_ms"),
                            format!("expected a non-negative integer, got {}", type_name(&other)),
                        );
                        return None;
                    }
                };
                let curve = read_curve(&opts_raw_get(&easing, "curve"), &format!("{key}.easing.curve"), errors)?;
                Some(Kind::Easing(EasingParams { duration_ms, curve }))
            }
            Value::Nil => None,
            other => {
                push_err(
                    errors,
                    key,
                    format!("expected `spring`, `easing` or `off`, got {}", type_name(&other)),
                );
                return None;
            }
        },
        other => {
            push_err(
                errors,
                key,
                format!("expected `spring`, `easing` or `off`, got {}", type_name(&other)),
            );
            return None;
        }
    };

    if off {
        let kind = kind.unwrap_or(Kind::Easing(EasingParams {
            duration_ms: 0,
            curve: Curve::Linear,
        }));
        return Some(Animation { off: true, kind });
    }

    let kind = match kind {
        Some(kind) => kind,
        None => {
            push_err(errors, key, "expected `spring`, `easing` or `off`");
            return None;
        }
    };

    Some(Animation { off: false, kind })
}

fn read_curve(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<Curve> {
    let Value::String(s) = value else {
        push_err(errors, key, format!("expected a string, got {}", type_name(value)));
        return None;
    };
    let s = s.to_string_lossy();

    match s.as_str() {
        "linear" => return Some(Curve::Linear),
        "ease-out-quad" => return Some(Curve::EaseOutQuad),
        "ease-out-cubic" => return Some(Curve::EaseOutCubic),
        "ease-out-expo" => return Some(Curve::EaseOutExpo),
        _ => {}
    }

    if let Some(rest) = s.strip_prefix("cubic-bezier(").and_then(|s| s.strip_suffix(')')) {
        let mut nums = rest
            .split([',', ' '])
            .filter(|p| !p.is_empty())
            .map(|p| p.parse::<f64>().ok());
        let mut vals = Vec::new();
        for _ in 0..4 {
            match nums.next() {
                Some(Some(v)) => vals.push(v),
                _ => {
                    push_err(errors, key, "invalid cubic-bezier curve");
                    return None;
                }
            }
        }
        if nums.next().is_some() {
            push_err(errors, key, "invalid cubic-bezier curve");
            return None;
        }
        return Some(Curve::CubicBezier(vals[0], vals[1], vals[2], vals[3]));
    }

    push_err(errors, key, format!("invalid curve `{s}`"));
    None
}

fn read_window_open_anim(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<WindowOpenAnim> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };
    let anim_value = match opts_raw_get(t, "anim") {
        Value::Nil => value.clone(),
        other => other,
    };
    let anim = read_animation(&anim_value, key, errors)?;
    let custom_shader = match opts_raw_get(t, "custom_shader") {
        Value::Nil => None,
        other => read_string(&other, &format!("{key}.custom_shader"), errors),
    };
    Some(WindowOpenAnim {
        anim,
        custom_shader,
    })
}

fn read_window_close_anim(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<WindowCloseAnim> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };
    let anim_value = match opts_raw_get(t, "anim") {
        Value::Nil => value.clone(),
        other => other,
    };
    let anim = read_animation(&anim_value, key, errors)?;
    let custom_shader = match opts_raw_get(t, "custom_shader") {
        Value::Nil => None,
        other => read_string(&other, &format!("{key}.custom_shader"), errors),
    };
    Some(WindowCloseAnim {
        anim,
        custom_shader,
    })
}

fn read_window_resize_anim(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<WindowResizeAnim> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };
    let anim_value = match opts_raw_get(t, "anim") {
        Value::Nil => value.clone(),
        other => other,
    };
    let anim = read_animation(&anim_value, key, errors)?;
    let custom_shader = match opts_raw_get(t, "custom_shader") {
        Value::Nil => None,
        other => read_string(&other, &format!("{key}.custom_shader"), errors),
    };
    Some(WindowResizeAnim {
        anim,
        custom_shader,
    })
}

fn apply_blur(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return;
        }
    };

    let mut part = BlurPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "off" => part.off = read_flag(&value),
            "on" => part.on = read_flag(&value),
            "passes" => part.passes = read_u8(&value, "blur.passes", errors),
            "offset" => part.offset = read_float_or_int(&value, "blur.offset", errors),
            "noise" => part.noise = read_float_or_int(&value, "blur.noise", errors),
            "saturation" => part.saturation = read_float_or_int(&value, "blur.saturation", errors),
            _ => push_err(errors, "blur", format!("unexpected key `{key}`")),
        }
    }

    config.blur.merge_with(&part);
}

fn apply_gestures(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return;
        }
    };

    let mut part = GesturesPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "dnd_edge_view_scroll" => {
                if let Some(p) = read_dnd_edge_view_scroll(&value, "gestures.dnd_edge_view_scroll", errors) {
                    part.dnd_edge_view_scroll = Some(p);
                }
            }
            "dnd_edge_workspace_switch" => {
                if let Some(p) = read_dnd_edge_workspace_switch(&value, "gestures.dnd_edge_workspace_switch", errors) {
                    part.dnd_edge_workspace_switch = Some(p);
                }
            }
            "hot_corners" => {
                if let Some(p) = read_hot_corners(&value, "gestures.hot_corners", errors) {
                    part.hot_corners = Some(p);
                }
            }
            _ => push_err(errors, "gestures", format!("unexpected key `{key}`")),
        }
    }

    config.gestures.merge_with(&part);
}

fn read_dnd_edge_view_scroll(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<DndEdgeViewScrollPart> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };
    let mut part = DndEdgeViewScrollPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "trigger_width" => part.trigger_width = read_float_or_int(&value, &format!("{key}.trigger_width"), errors),
            "delay_ms" => part.delay_ms = read_u16(&value, &format!("{key}.delay_ms"), errors),
            "max_speed" => part.max_speed = read_float_or_int(&value, &format!("{key}.max_speed"), errors),
            _ => push_err(errors, &key, format!("unexpected key `{key}`")),
        }
    }
    Some(part)
}

fn read_dnd_edge_workspace_switch(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<DndEdgeWorkspaceSwitchPart> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };
    let mut part = DndEdgeWorkspaceSwitchPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "trigger_height" => part.trigger_height = read_float_or_int(&value, &format!("{key}.trigger_height"), errors),
            "delay_ms" => part.delay_ms = read_u16(&value, &format!("{key}.delay_ms"), errors),
            "max_speed" => part.max_speed = read_float_or_int(&value, &format!("{key}.max_speed"), errors),
            _ => push_err(errors, &key, format!("unexpected key `{key}`")),
        }
    }
    Some(part)
}

fn read_hot_corners(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<HotCorners> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };
    let mut corners = HotCorners::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "off" => corners.off = read_flag(&value),
            "top_left" => corners.top_left = read_flag(&value),
            "top_right" => corners.top_right = read_flag(&value),
            "bottom_left" => corners.bottom_left = read_flag(&value),
            "bottom_right" => corners.bottom_right = read_flag(&value),
            _ => push_err(errors, &key, format!("unexpected key `{key}`")),
        }
    }
    Some(corners)
}

fn apply_overview(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return;
        }
    };

    let mut part = OverviewPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "zoom" => part.zoom = read_float_or_int(&value, "overview.zoom", errors),
            "backdrop_color" => part.backdrop_color = read_color(&value, "overview.backdrop_color", errors),
            "workspace_shadow" => part.workspace_shadow = read_workspace_shadow_part(&value, "overview.workspace_shadow", errors),
            _ => push_err(errors, "overview", format!("unexpected key `{key}`")),
        }
    }

    config.overview.merge_with(&part);
}

fn read_workspace_shadow_part(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<WorkspaceShadowPart> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };
    let mut part = WorkspaceShadowPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "off" => part.off = read_flag(&value),
            "on" => part.on = read_flag(&value),
            "offset" => part.offset = read_shadow_offset(&value, &format!("{key}.offset"), errors),
            "softness" => part.softness = read_float_or_int(&value, &format!("{key}.softness"), errors),
            "spread" => part.spread = read_float_or_int(&value, &format!("{key}.spread"), errors),
            "color" => part.color = read_color(&value, &format!("{key}.color"), errors),
            _ => push_err(errors, &key, format!("unexpected key `{key}`")),
        }
    }
    Some(part)
}

fn apply_xwayland_satellite(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return;
        }
    };

    let mut part = XwaylandSatellitePart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "off" => part.off = read_flag(&value),
            "on" => part.on = read_flag(&value),
            "path" => part.path = read_string(&value, "xwayland_satellite.path", errors),
            _ => push_err(errors, "xwayland_satellite", format!("unexpected key `{key}`")),
        }
    }

    config.xwayland_satellite.merge_with(&part);
}

fn apply_switch_events(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return;
        }
    };

    let mut part = SwitchBinds::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "lid_open" => part.lid_open = read_switch_action(&value, "switch_events.lid_open", errors),
            "lid_close" => part.lid_close = read_switch_action(&value, "switch_events.lid_close", errors),
            "tablet_mode_on" => part.tablet_mode_on = read_switch_action(&value, "switch_events.tablet_mode_on", errors),
            "tablet_mode_off" => part.tablet_mode_off = read_switch_action(&value, "switch_events.tablet_mode_off", errors),
            _ => push_err(errors, "switch_events", format!("unexpected key `{key}`")),
        }
    }

    config.switch_events.merge_with(&part);
}

fn read_switch_action(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<SwitchAction> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };

    match opts_raw_get(t, "spawn") {
        Value::Nil => {
            push_err(errors, key, "expected `spawn`");
            None
        }
        Value::String(s) => Some(SwitchAction {
            spawn: vec![s.to_string_lossy()],
        }),
        other => {
            let spawn = read_list_of_strings2(&other, &format!("{key}.spawn"), errors);
            Some(SwitchAction { spawn })
        }
    }
}

fn read_list_of_strings2(value: &Value, key: &str, errors: &mut Vec<String>) -> Vec<String> {
    match value {
        Value::Table(t) => {
            let mut rv = Vec::new();
            for i in 1..=t.raw_len() {
                match t.raw_get::<Value>(i as i64) {
                    Ok(Value::String(s)) => {
                        if let Ok(s) = s.to_str() {
                            rv.push(s.to_string());
                        }
                    }
                    Ok(other) => {
                        push_err(
                            errors,
                            key,
                            format!("expected a list of strings, got {}", type_name(&other)),
                        );
                        return Vec::new();
                    }
                    Err(_) => break,
                }
            }
            rv
        }
        _ => {
            push_err(errors, key, format!("expected a list of strings, got {}", type_name(value)));
            Vec::new()
        }
    }
}

fn apply_debug(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return;
        }
    };

    let mut part = DebugPart::default();
    let flag = |field: &mut Option<Flag>, value: &Value| {
        *field = read_flag_value(value);
    };

    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "preview_render" => match value {
                Value::String(s) => match s.to_str().as_deref() {
                    Ok("screencast") => part.preview_render = Some(PreviewRender::Screencast),
                    Ok("screen-capture") => part.preview_render = Some(PreviewRender::ScreenCapture),
                    _ => push_err(errors, "debug.preview_render", "invalid value"),
                },
                other => push_err(errors, "debug.preview_render", format!("expected a string, got {}", type_name(&other))),
            },
            "dbus_interfaces_in_non_session_instances" => flag(&mut part.dbus_interfaces_in_non_session_instances, &value),
            "wait_for_frame_completion_before_queueing" => flag(&mut part.wait_for_frame_completion_before_queueing, &value),
            "enable_overlay_planes" => flag(&mut part.enable_overlay_planes, &value),
            "disable_cursor_plane" => flag(&mut part.disable_cursor_plane, &value),
            "disable_direct_scanout" => flag(&mut part.disable_direct_scanout, &value),
            "restrict_primary_scanout_to_matching_format" => flag(&mut part.restrict_primary_scanout_to_matching_format, &value),
            "force_disable_connectors_on_resume" => flag(&mut part.force_disable_connectors_on_resume, &value),
            "render_drm_device" => {
                if let Some(s) = read_string(&value, "debug.render_drm_device", errors) {
                    part.render_drm_device = Some(PathBuf::from(s));
                }
            }
            "ignored_drm_devices" => {
                let devices = read_list_of_strings2(&value, "debug.ignored_drm_devices", errors);
                part.ignored_drm_devices = devices.into_iter().map(PathBuf::from).collect();
            }
            "force_pipewire_invalid_modifier" => flag(&mut part.force_pipewire_invalid_modifier, &value),
            "emulate_zero_presentation_time" => flag(&mut part.emulate_zero_presentation_time, &value),
            "disable_resize_throttling" => flag(&mut part.disable_resize_throttling, &value),
            "disable_transactions" => flag(&mut part.disable_transactions, &value),
            "keep_laptop_panel_on_when_lid_is_closed" => flag(&mut part.keep_laptop_panel_on_when_lid_is_closed, &value),
            "disable_monitor_names" => flag(&mut part.disable_monitor_names, &value),
            "strict_new_window_focus_policy" => flag(&mut part.strict_new_window_focus_policy, &value),
            "honor_xdg_activation_with_invalid_serial" => flag(&mut part.honor_xdg_activation_with_invalid_serial, &value),
            "deactivate_unfocused_windows" => flag(&mut part.deactivate_unfocused_windows, &value),
            "skip_cursor_only_updates_during_vrr" => flag(&mut part.skip_cursor_only_updates_during_vrr, &value),
            "disable_10bit_output" => flag(&mut part.disable_10bit_output, &value),
            _ => push_err(errors, "debug", format!("unexpected key `{key}`")),
        }
    }

    config.debug.merge_with(&part);
}

// ===============================================================================================
// Layout / appearance
// ===============================================================================================

fn apply_layout(
    config: &mut Config,
    key: &str,
    value: &Value,
    errors: &mut Vec<String>,
    recursion: u8,
) {
    let Some(mut part) = read_layout_part(value, key, errors) else {
        return;
    };

    // Preserve the behavior we'd always had for the border section (see the comment in the old
    // parser): `layout {}` gives border = off, `layout { border {} }` gives border = on, and
    // `layout { border { off } }` gives border = off. Only applied to the main config file.
    if recursion == 0 {
        if let Some(border) = part.border.as_mut() {
            if !border.on && !border.off {
                border.on = true;
            }
        }
    }

    config.layout.merge_with(&part);
}

fn read_layout_part(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<LayoutPart> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };

    let mut part = LayoutPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "focus_ring" => part.focus_ring = read_border_rule(&value, &format!("{key}.focus_ring"), errors),
            "border" => part.border = read_border_rule(&value, &format!("{key}.border"), errors),
            "shadow" => part.shadow = read_shadow_rule(&value, &format!("{key}.shadow"), errors),
            "insert_hint" => part.insert_hint = read_insert_hint_part(&value, &format!("{key}.insert_hint"), errors),
            "preset_column_widths" => part.preset_column_widths = read_preset_sizes(&value, &format!("{key}.preset_column_widths"), errors),
            "default_column_width" => part.default_column_width = read_default_preset_size(&value, &format!("{key}.default_column_width"), errors),
            "preset_window_heights" => part.preset_window_heights = read_preset_sizes(&value, &format!("{key}.preset_window_heights"), errors),
            "center_focused_column" => part.center_focused_column = read_center_focused_column(&value, &format!("{key}.center_focused_column"), errors),
            "always_center_single_column" => part.always_center_single_column = read_flag_value(&value),
            "empty_workspace_above_first" => part.empty_workspace_above_first = read_flag_value(&value),
            "default_column_display" => part.default_column_display = read_enum(&value, &format!("{key}.default_column_display"), errors),
            "dwindle_windows_per_column" => part.dwindle_windows_per_column = read_positive_usize(&value, &format!("{key}.dwindle_windows_per_column"), errors),
            "gaps" => part.gaps = read_float_or_int(&value, &format!("{key}.gaps"), errors),
            "struts" => part.struts = read_struts(&value, &format!("{key}.struts"), errors),
            "background_color" => part.background_color = read_color(&value, &format!("{key}.background_color"), errors),
            _ => push_err(errors, &key, format!("unexpected key `{key}`")),
        }
    }
    Some(part)
}

fn read_border_rule(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<BorderRule> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };

    let mut part = BorderRule::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "off" => part.off = read_flag(&value),
            "on" => part.on = read_flag(&value),
            "width" => part.width = read_float_or_int(&value, &format!("{key}.width"), errors),
            "active_color" => part.active_color = read_color(&value, &format!("{key}.active_color"), errors),
            "inactive_color" => part.inactive_color = read_color(&value, &format!("{key}.inactive_color"), errors),
            "urgent_color" => part.urgent_color = read_color(&value, &format!("{key}.urgent_color"), errors),
            "active_gradient" => part.active_gradient = read_gradient(&value, &format!("{key}.active_gradient"), errors),
            "inactive_gradient" => part.inactive_gradient = read_gradient(&value, &format!("{key}.inactive_gradient"), errors),
            "urgent_gradient" => part.urgent_gradient = read_gradient(&value, &format!("{key}.urgent_gradient"), errors),
            _ => push_err(errors, &key, format!("unexpected key `{key}`")),
        }
    }
    Some(part)
}

fn read_shadow_rule(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<ShadowRule> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };

    let mut part = ShadowRule::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "off" => part.off = read_flag(&value),
            "on" => part.on = read_flag(&value),
            "offset" => part.offset = read_shadow_offset(&value, &format!("{key}.offset"), errors),
            "softness" => part.softness = read_float_or_int(&value, &format!("{key}.softness"), errors),
            "spread" => part.spread = read_float_or_int(&value, &format!("{key}.spread"), errors),
            "draw_behind_window" => part.draw_behind_window = read_bool_or_flag(&value),
            "color" => part.color = read_color(&value, &format!("{key}.color"), errors),
            "inactive_color" => part.inactive_color = read_color(&value, &format!("{key}.inactive_color"), errors),
            _ => push_err(errors, &key, format!("unexpected key `{key}`")),
        }
    }
    Some(part)
}

fn read_insert_hint_part(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<InsertHintPart> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };

    let mut part = InsertHintPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "off" => part.off = read_flag(&value),
            "on" => part.on = read_flag(&value),
            "color" => part.color = read_color(&value, &format!("{key}.color"), errors),
            "gradient" => part.gradient = read_gradient(&value, &format!("{key}.gradient"), errors),
            _ => push_err(errors, &key, format!("unexpected key `{key}`")),
        }
    }
    Some(part)
}

fn read_preset_sizes(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<Vec<PresetSize>> {
    let values = read_list_of_tables(value, key, errors)?;
    let mut rv = Vec::new();
    for item in values {
        rv.push(read_preset_size(&Value::Table(item), key, errors)?);
    }
    Some(rv)
}

fn read_default_preset_size(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<DefaultPresetSize> {
    match value {
        Value::Nil => Some(DefaultPresetSize(None)),
        Value::Table(t) => {
            // An empty table means "let the window decide"; anything else must be a preset size.
            if t.raw_len() == 0 && t.pairs::<String, Value>().next().is_none() {
                return Some(DefaultPresetSize(None));
            }
            read_preset_size(value, key, errors).map(|size| DefaultPresetSize(Some(size)))
        }
        _ => read_preset_size(value, key, errors).map(|size| DefaultPresetSize(Some(size))),
    }
}

fn read_preset_size(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<PresetSize> {
    match value {
        Value::Integer(i) => Some(PresetSize::Fixed(*i as i32)),
        Value::Number(n) => Some(PresetSize::Proportion(*n)),
        Value::Table(t) => {
            match opts_raw_get(t, "proportion") {
                Value::Nil => {}
                other => {
                    return read_f64(&other, &format!("{key}.proportion"), errors)
                        .map(PresetSize::Proportion);
                }
            }
            match opts_raw_get(t, "fixed") {
                Value::Nil => {}
                other => {
                    let fixed = read_i32(&other, &format!("{key}.fixed"), errors)?;
                    return Some(PresetSize::Fixed(fixed));
                }
            }
            push_err(errors, key, "expected `proportion` or `fixed`");
            None
        }
        _ => {
            push_err(errors, key, "expected a number or a table");
            None
        }
    }
}

fn read_center_focused_column(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<CenterFocusedColumn> {
    let Value::String(s) = value else {
        push_err(errors, key, format!("expected a string, got {}", type_name(value)));
        return None;
    };
    match s.to_str().as_deref() {
        Ok("never") => Some(CenterFocusedColumn::Never),
        Ok("always") => Some(CenterFocusedColumn::Always),
        Ok("on-overflow") => Some(CenterFocusedColumn::OnOverflow),
        _ => {
            push_err(errors, key, "must be \"never\", \"always\" or \"on-overflow\"");
            None
        }
    }
}

/// Both branches of the `any` check return identical `Some(struts)`; collapsing it would orphan
/// the `any` flag used by the per-field closure, so keep the code as-is.
#[allow(clippy::if_same_then_else)]
fn read_struts(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<Struts> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };

    let mut struts = Struts {
        left: FloatOrInt(0.),
        right: FloatOrInt(0.),
        top: FloatOrInt(0.),
        bottom: FloatOrInt(0.),
    };
    let mut any = false;
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        let mut set = |field: &mut FloatOrInt<-65535, 65535>, value: &Value| {
            if let Some(v) = read_float_or_int(value, &format!("struts.{key}"), errors) {
                *field = v;
                any = true;
            }
        };
        match key.as_str() {
            "left" => set(&mut struts.left, &value),
            "right" => set(&mut struts.right, &value),
            "top" => set(&mut struts.top, &value),
            "bottom" => set(&mut struts.bottom, &value),
            _ => push_err(errors, "struts", format!("unexpected key `{key}`")),
        }
    }
    if any {
        Some(struts)
    } else {
        Some(struts)
    }
}

// ===============================================================================================
// Outputs
// ===============================================================================================

fn apply_outputs(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let items = match read_list_of_tables(value, key, errors) {
        Some(items) => items,
        None => return,
    };

    for item in items {
        if let Some(output) = read_output(&item, errors) {
            config.outputs.0.push(output);
        }
    }
}

fn read_output(t: &Table, errors: &mut Vec<String>) -> Option<Output> {
    let name = read_string(&opts_raw_get(t, "name").clone(), "output.name", errors)?;

    let mut output = Output {
        name,
        ..Default::default()
    };

    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        if key == "name" {
            continue;
        }
        match key.as_str() {
            "off" => output.off = read_flag(&value),
            "scale" => output.scale = read_float_or_int(&value, "output.scale", errors),
            "transform" => output.transform = read_enum(&value, "output.transform", errors).unwrap_or(Transform::Normal),
            "position" => output.position = read_output_position(&value, "output.position", errors),
            "max_bpc" => output.max_bpc = read_max_bpc(&value, "output.max_bpc", errors),
            "mode" => output.mode = read_mode(&value, "output.mode", errors),
            "modeline" => output.modeline = read_modeline(&value, "output.modeline", errors),
            "variable_refresh_rate" => output.variable_refresh_rate = read_vrr(&value, "output.variable_refresh_rate", errors),
            "focus_at_startup" => output.focus_at_startup = read_flag(&value),
            "background_color" => output.background_color = read_color(&value, "output.background_color", errors),
            "backdrop_color" => output.backdrop_color = read_color(&value, "output.backdrop_color", errors),
            "hot_corners" => output.hot_corners = read_hot_corners(&value, "output.hot_corners", errors),
            "layout" => output.layout = read_layout_part(&value, "output.layout", errors),
            _ => push_err(errors, "output", format!("unexpected key `{key}`")),
        }
    }

    Some(output)
}

fn read_output_position(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<Position> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };
    let x = read_i32(&opts_raw_get(t, "x").clone(), &format!("{key}.x"), errors)?;
    let y = read_i32(&opts_raw_get(t, "y").clone(), &format!("{key}.y"), errors)?;
    Some(Position { x, y })
}

fn read_max_bpc(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<ConfigMaxBpc> {
    read_enum::<ymir_ipc::MaxBpc>(value, key, errors).map(ConfigMaxBpc)
}

fn read_mode(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<Mode> {
    match value {
        Value::String(s) => {
            let mode = match s.to_str().map(|s| s.parse::<ConfiguredMode>()) {
                Ok(Ok(mode)) => mode,
                _ => {
                    push_err(errors, key, format!("invalid mode `{}`", s.to_string_lossy()));
                    return None;
                }
            };
            Some(Mode { custom: false, mode })
        }
        Value::Table(t) => {
            let mode = read_mode_config(t, key, errors)?;
            let custom = read_flag_default(&opts_raw_get(t, "custom"), false);
            Some(Mode { custom, mode })
        }
        _ => {
            push_err(errors, key, format!("expected a string or a table, got {}", type_name(value)));
            None
        }
    }
}

fn read_mode_config(t: &Table, key: &str, errors: &mut Vec<String>) -> Option<ConfiguredMode> {
    match opts_raw_get(t, "mode") {
        Value::String(s) => match s.to_str().map(|s| s.parse::<ConfiguredMode>()) {
            Ok(Ok(mode)) => Some(mode),
            _ => {
                push_err(errors, key, format!("invalid mode `{}`", s.to_string_lossy()));
                None
            }
        },
        other => {
            push_err(errors, key, format!("expected a mode string, got {}", type_name(&other)));
            None
        }
    }
}

fn read_modeline(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<Modeline> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };

    let mut modeline = Modeline {
        clock: 0.,
        hdisplay: 0,
        hsync_start: 0,
        hsync_end: 0,
        htotal: 0,
        vdisplay: 0,
        vsync_start: 0,
        vsync_end: 0,
        vtotal: 0,
        hsync_polarity: ymir_ipc::HSyncPolarity::PHSync,
        vsync_polarity: ymir_ipc::VSyncPolarity::PVSync,
    };

    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "clock" => modeline.clock = read_f64(&value, &format!("{key}.clock"), errors)?,
            "hdisplay" => modeline.hdisplay = read_u16(&value, &format!("{key}.hdisplay"), errors)?,
            "hsync_start" => modeline.hsync_start = read_u16(&value, &format!("{key}.hsync_start"), errors)?,
            "hsync_end" => modeline.hsync_end = read_u16(&value, &format!("{key}.hsync_end"), errors)?,
            "htotal" => modeline.htotal = read_u16(&value, &format!("{key}.htotal"), errors)?,
            "vdisplay" => modeline.vdisplay = read_u16(&value, &format!("{key}.vdisplay"), errors)?,
            "vsync_start" => modeline.vsync_start = read_u16(&value, &format!("{key}.vsync_start"), errors)?,
            "vsync_end" => modeline.vsync_end = read_u16(&value, &format!("{key}.vsync_end"), errors)?,
            "vtotal" => modeline.vtotal = read_u16(&value, &format!("{key}.vtotal"), errors)?,
            "hsync_polarity" => modeline.hsync_polarity = read_enum(&value, &format!("{key}.hsync_polarity"), errors)?,
            "vsync_polarity" => modeline.vsync_polarity = read_enum(&value, &format!("{key}.vsync_polarity"), errors)?,
            _ => push_err(errors, &key, format!("unexpected key `{key}`")),
        }
    }

    Some(modeline)
}

fn read_vrr(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<Vrr> {
    match value {
        Value::Boolean(on) => Some(Vrr { on_demand: !on }),
        Value::Table(t) => Some(Vrr {
            on_demand: read_flag_default(&opts_raw_get(t, "on_demand"), false),
        }),
        _ => {
            push_err(errors, key, format!("expected a boolean or a table, got {}", type_name(value)));
            None
        }
    }
}

// ===============================================================================================
// Spawn / misc
// ===============================================================================================

fn apply_spawn_at_startup(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let items = match read_list_of_tables(value, key, errors) {
        Some(items) => items,
        None => return,
    };

    for item in items {
        let command = match opts_raw_get(&item, "command") {
            Value::Nil => {
                push_err(errors, "spawn_at_startup", "missing `command`");
                continue;
            }
            other => read_list_of_strings2(&other, "spawn_at_startup.command", errors),
        };
        config.spawn_at_startup.push(SpawnAtStartup { command });
    }
}

fn apply_spawn_sh_at_startup(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let items = match read_list_of_tables(value, key, errors) {
        Some(items) => items,
        None => return,
    };

    for item in items {
        let command = match opts_raw_get(&item, "command") {
            Value::Nil => {
                push_err(errors, "spawn_sh_at_startup", "missing `command`");
                continue;
            }
            other => match read_string(&other, "spawn_sh_at_startup.command", errors) {
                Some(s) => s,
                None => continue,
            },
        };
        config.spawn_sh_at_startup.push(SpawnShAtStartup { command });
    }
}

fn apply_prefer_no_csd(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    config.prefer_no_csd = match value {
        Value::Boolean(b) => *b,
        _ => {
            push_err(errors, key, format!("expected a boolean, got {}", type_name(value)));
            return;
        }
    };
}

fn apply_screenshot_path(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    match value {
        Value::String(s) => {
            config.screenshot_path = ScreenshotPath(Some(s.to_string_lossy()));
        }
        _ => {
            push_err(errors, key, format!("expected a string, got {}", type_name(value)));
        }
    }
}

fn apply_environment(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return;
        }
    };

    if t.raw_len() > 0 {
        for i in 1..=t.raw_len() {
            match t.raw_get::<Value>(i as i64) {
                Ok(Value::Table(row)) => {
                    let name = match opts_raw_get(&row, "name") {
                        Value::String(s) => s.to_string_lossy(),
                        other => {
                            push_err(
                                errors,
                                "environment",
                                format!("`name` must be a string, got {}", type_name(&other)),
                            );
                            continue;
                        }
                    };
                    let value = match opts_raw_get(&row, "value") {
                        Value::String(s) => Some(s.to_string_lossy()),
                        Value::Nil => None,
                        Value::LightUserData(u) if u.0.is_null() => None,
                        other => {
                            push_err(
                                errors,
                                "environment",
                                format!("`value` must be a string, got {}", type_name(&other)),
                            );
                            continue;
                        }
                    };
                    config.environment.0.push(EnvironmentVariable { name, value });
                }
                Ok(other) => {
                    push_err(
                        errors,
                        "environment",
                        format!("expected a list of tables, got {} at index {i}", type_name(&other)),
                    );
                    return;
                }
                Err(_) => break,
            }
        }
        return;
    }

    let iter = t.pairs::<String, Value>();
    for kv in iter {
        let (name, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let name = normalize_key(&name);
        let value = match value {
            Value::String(s) => Some(s.to_string_lossy()),
            Value::Nil => None,
            Value::LightUserData(u) if u.0.is_null() => None,
            other => {
                push_err(
                    errors,
                    "environment",
                    format!("`{name}` must be a string, got {}", type_name(&other)),
                );
                None
            }
        };
        config.environment.0.push(EnvironmentVariable { name, value });
    }
}

// ===============================================================================================
// Rules
// ===============================================================================================

fn apply_window_rules(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let items = match read_list_of_tables(value, key, errors) {
        Some(items) => items,
        None => return,
    };

    for item in items {
        if let Some(rule) = read_window_rule(&item, errors) {
            config.window_rules.push(rule);
        }
    }
}

fn read_window_rule(t: &Table, errors: &mut Vec<String>) -> Option<WindowRule> {
    let mut rule = WindowRule::default();

    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "match" => rule.matches = read_match_list(&value, "window_rules.match", errors),
            "exclude" => rule.excludes = read_match_list(&value, "window_rules.exclude", errors),
            "default_column_width" => rule.default_column_width = read_default_preset_size(&value, "window_rules.default_column_width", errors),
            "default_window_height" => rule.default_window_height = read_default_preset_size(&value, "window_rules.default_window_height", errors),
            "open_on_output" => rule.open_on_output = read_string(&value, "window_rules.open_on_output", errors),
            "open_on_workspace" => rule.open_on_workspace = read_string(&value, "window_rules.open_on_workspace", errors),
            "open_maximized" => rule.open_maximized = read_optional_bool2(&value),
            "open_maximized_to_edges" => rule.open_maximized_to_edges = read_optional_bool2(&value),
            "open_fullscreen" => rule.open_fullscreen = read_optional_bool2(&value),
            "open_floating" => rule.open_floating = read_optional_bool2(&value),
            "open_focused" => rule.open_focused = read_optional_bool2(&value),
            "on_xdg_activate" => rule.on_xdg_activate = read_on_xdg_activate(&value, "window_rules.on_xdg_activate", errors),
            "min_width" => rule.min_width = read_u16(&value, "window_rules.min_width", errors),
            "min_height" => rule.min_height = read_u16(&value, "window_rules.min_height", errors),
            "max_width" => rule.max_width = read_u16(&value, "window_rules.max_width", errors),
            "max_height" => rule.max_height = read_u16(&value, "window_rules.max_height", errors),
            "focus_ring" => rule.focus_ring = read_border_rule(&value, "window_rules.focus_ring", errors).unwrap_or_default(),
            "border" => rule.border = read_border_rule(&value, "window_rules.border", errors).unwrap_or_default(),
            "shadow" => rule.shadow = read_shadow_rule(&value, "window_rules.shadow", errors).unwrap_or_default(),
            "draw_border_with_background" => rule.draw_border_with_background = read_optional_bool2(&value),
            "opacity" => rule.opacity = read_opacity(&value, "window_rules.opacity", errors),
            "geometry_corner_radius" => rule.geometry_corner_radius = read_corner_radius(&value, "window_rules.geometry_corner_radius", errors),
            "clip_to_geometry" => rule.clip_to_geometry = read_optional_bool2(&value),
            "baba_is_float" => rule.baba_is_float = read_optional_bool2(&value),
            "block_out_from" => rule.block_out_from = read_block_out_from(&value, "window_rules.block_out_from", errors),
            "variable_refresh_rate" => rule.variable_refresh_rate = read_optional_bool2(&value),
            "default_column_display" => rule.default_column_display = read_enum(&value, "window_rules.default_column_display", errors),
            "default_floating_position" => rule.default_floating_position = read_floating_position(&value, "window_rules.default_floating_position", errors),
            "scroll_factor" => rule.scroll_factor = read_float_or_int(&value, "window_rules.scroll_factor", errors),
            "tiled_state" => rule.tiled_state = read_optional_bool2(&value),
            "background_effect" => rule.background_effect = read_background_effect_rule(&value, "window_rules.background_effect", errors).unwrap_or_default(),
            "popups" => rule.popups = read_popups_rule(&value, "window_rules.popups", errors).unwrap_or_default(),
            _ => push_err(errors, "window_rules", format!("unexpected key `{key}`")),
        }
    }

    Some(rule)
}

fn read_match_list(value: &Value, key: &str, errors: &mut Vec<String>) -> Vec<WindowMatch> {
    let Some(items) = read_list_of_tables(value, key, errors) else {
        return Vec::new();
    };

    let mut rv = Vec::new();
    for item in items {
        if let Some(m) = read_window_match(&item, key, errors) {
            rv.push(m);
        }
    }
    rv
}

fn read_window_match(t: &Table, _key: &str, errors: &mut Vec<String>) -> Option<WindowMatch> {
    let mut m = WindowMatch::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "app_id" => m.app_id = read_regex(&value, &format!("{key}.app_id"), errors),
            "title" => m.title = read_regex(&value, &format!("{key}.title"), errors),
            "is_active" => m.is_active = read_optional_bool2(&value),
            "is_focused" => m.is_focused = read_optional_bool2(&value),
            "is_active_in_column" => m.is_active_in_column = read_optional_bool2(&value),
            "is_floating" => m.is_floating = read_optional_bool2(&value),
            "is_window_cast_target" => m.is_window_cast_target = read_optional_bool2(&value),
            "is_urgent" => m.is_urgent = read_optional_bool2(&value),
            "at_startup" => m.at_startup = read_optional_bool2(&value),
            _ => push_err(errors, &key, format!("unexpected key `{key}`")),
        }
    }
    Some(m)
}

fn read_regex(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<RegexEq> {
    match value {
        Value::String(s) => match s.to_str().map(|s| RegexEq::from_str(s.as_ref())) {
            Ok(Ok(r)) => Some(r),
            _ => {
                push_err(errors, key, format!("invalid regex `{}`", s.to_string_lossy()));
                None
            }
        },
        _ => {
            push_err(errors, key, format!("expected a string, got {}", type_name(value)));
            None
        }
    }
}

fn read_optional_bool2(value: &Value) -> Option<bool> {
    match value {
        Value::Nil => None,
        Value::Boolean(b) => Some(*b),
        _ => None,
    }
}

fn read_on_xdg_activate(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<OnXdgActivate> {
    let Value::String(s) = value else {
        push_err(errors, key, format!("expected a string, got {}", type_name(value)));
        return None;
    };
    match s.to_str().as_deref() {
        Ok("ignore") => Some(OnXdgActivate::Ignore),
        Ok("set-urgent") => Some(OnXdgActivate::SetUrgent),
        Ok("focus") => Some(OnXdgActivate::Focus),
        _ => {
            push_err(errors, key, "must be \"ignore\", \"set-urgent\" or \"focus\"");
            None
        }
    }
}

fn read_opacity(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<f32> {
    match value {
        Value::Integer(i) => Some(*i as f32),
        Value::Number(n) => Some(*n as f32),
        _ => {
            push_err(errors, key, format!("expected a number, got {}", type_name(value)));
            None
        }
    }
}

fn read_block_out_from(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<BlockOutFrom> {
    let Value::String(s) = value else {
        push_err(errors, key, format!("expected a string, got {}", type_name(value)));
        return None;
    };
    match s.to_str().as_deref() {
        Ok("screencast") => Some(BlockOutFrom::Screencast),
        Ok("screen-capture") => Some(BlockOutFrom::ScreenCapture),
        _ => {
            push_err(errors, key, "must be \"screencast\" or \"screen-capture\"");
            None
        }
    }
}

fn read_floating_position(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<FloatingPosition> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };
    let x = read_float_or_int(&opts_raw_get(t, "x").clone(), &format!("{key}.x"), errors)?;
    let y = read_float_or_int(&opts_raw_get(t, "y").clone(), &format!("{key}.y"), errors)?;
    let relative_to = match opts_raw_get(t, "relative_to") {
        Value::Nil => RelativeTo::TopLeft,
        Value::String(s) => match s.to_str().as_deref() {
            Ok("top-left") => RelativeTo::TopLeft,
            Ok("top-right") => RelativeTo::TopRight,
            Ok("bottom-left") => RelativeTo::BottomLeft,
            Ok("bottom-right") => RelativeTo::BottomRight,
            Ok("top") => RelativeTo::Top,
            Ok("bottom") => RelativeTo::Bottom,
            Ok("left") => RelativeTo::Left,
            Ok("right") => RelativeTo::Right,
            _ => {
                push_err(errors, key, "invalid `relative_to`");
                return None;
            }
        },
        other => {
            push_err(errors, key, format!("`relative_to` must be a string, got {}", type_name(&other)));
            return None;
        }
    };
    Some(FloatingPosition { x, y, relative_to })
}

fn read_background_effect_rule(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<BackgroundEffectRule> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };

    let mut part = BackgroundEffectRule::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "xray" => part.xray = read_optional_bool2(&value),
            "blur" => part.blur = read_optional_bool2(&value),
            "noise" => part.noise = read_float_or_int(&value, &format!("{key}.noise"), errors),
            "saturation" => part.saturation = read_float_or_int(&value, &format!("{key}.saturation"), errors),
            _ => push_err(errors, &key, format!("unexpected key `{key}`")),
        }
    }

    Some(part)
}

fn read_popups_rule(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<PopupsRule> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };

    let mut part = PopupsRule::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "opacity" => part.opacity = read_opacity(&value, &format!("{key}.opacity"), errors),
            "geometry_corner_radius" => part.geometry_corner_radius = read_corner_radius(&value, &format!("{key}.geometry_corner_radius"), errors),
            "background_effect" => part.background_effect = read_background_effect_rule(&value, &format!("{key}.background_effect"), errors).unwrap_or_default(),
            _ => push_err(errors, &key, format!("unexpected key `{key}`")),
        }
    }

    Some(part)
}

fn apply_layer_rules(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let items = match read_list_of_tables(value, key, errors) {
        Some(items) => items,
        None => return,
    };

    for item in items {
        if let Some(rule) = read_layer_rule(&item, errors) {
            config.layer_rules.push(rule);
        }
    }
}

fn read_layer_rule(t: &Table, errors: &mut Vec<String>) -> Option<LayerRule> {
    let mut rule = LayerRule::default();

    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "match" => rule.matches = read_layer_match_list(&value, "layer_rules.match", errors),
            "exclude" => rule.excludes = read_layer_match_list(&value, "layer_rules.exclude", errors),
            "opacity" => rule.opacity = read_opacity(&value, "layer_rules.opacity", errors),
            "block_out_from" => rule.block_out_from = read_block_out_from(&value, "layer_rules.block_out_from", errors),
            "shadow" => rule.shadow = read_shadow_rule(&value, "layer_rules.shadow", errors).unwrap_or_default(),
            "geometry_corner_radius" => rule.geometry_corner_radius = read_corner_radius(&value, "layer_rules.geometry_corner_radius", errors),
            "place_within_backdrop" => rule.place_within_backdrop = read_optional_bool2(&value),
            "baba_is_float" => rule.baba_is_float = read_optional_bool2(&value),
            "background_effect" => rule.background_effect = read_background_effect_rule(&value, "layer_rules.background_effect", errors).unwrap_or_default(),
            "popups" => rule.popups = read_popups_rule(&value, "layer_rules.popups", errors).unwrap_or_default(),
            _ => push_err(errors, "layer_rules", format!("unexpected key `{key}`")),
        }
    }

    Some(rule)
}

fn read_layer_match_list(value: &Value, key: &str, errors: &mut Vec<String>) -> Vec<crate::layer_rule::Match> {
    let Some(items) = read_list_of_tables(value, key, errors) else {
        return Vec::new();
    };

    let mut rv = Vec::new();
    for item in items {
        if let Some(m) = read_layer_match(&item, key, errors) {
            rv.push(m);
        }
    }
    rv
}

fn read_layer_match(t: &Table, _key: &str, errors: &mut Vec<String>) -> Option<crate::layer_rule::Match> {
    let mut m = crate::layer_rule::Match::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "namespace" => m.namespace = read_regex(&value, &format!("{key}.namespace"), errors),
            "at_startup" => m.at_startup = read_optional_bool2(&value),
            "layer" => m.layer = read_enum(&value, &format!("{key}.layer"), errors),
            _ => push_err(errors, &key, format!("unexpected key `{key}`")),
        }
    }
    Some(m)
}

// ===============================================================================================
// Workspaces / binds / recent windows
// ===============================================================================================

fn apply_workspaces(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let items = match read_list_of_tables(value, key, errors) {
        Some(items) => items,
        None => return,
    };

    for item in items {
        if let Some(workspace) = read_workspace(&item, errors) {
            config.workspaces.push(workspace);
        }
    }
}

fn read_workspace(t: &Table, errors: &mut Vec<String>) -> Option<Workspace> {
    let name = read_string(&opts_raw_get(t, "name").clone(), "workspaces.name", errors)?;

    let mut workspace = Workspace {
        name: WorkspaceName(name),
        open_on_output: None,
        layout: None,
    };

    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "name" => {}
            "open_on_output" => workspace.open_on_output = read_string(&value, "workspaces.open_on_output", errors),
            "layout" => {
                workspace.layout =
                    read_layout_part(&value, "workspaces.layout", errors).map(WorkspaceLayoutPart);
            }
            _ => push_err(errors, "workspaces", format!("unexpected key `{key}`")),
        }
    }

    Some(workspace)
}

fn apply_binds(config: &mut Config, key: &str, value: &Value, errors: &mut Vec<String>) {
    let Some(items) = read_list_of_tables(value, key, errors) else {
        return;
    };

    for item in items {
        if let Some(bind) = parse_bind("", &item, errors) {
            // We replace conflicting binds, rather than error, to support overriding imported
            // configs.
            config.binds.0.retain(|existing| existing.key != bind.key);
            config.binds.0.push(bind);
        }
    }
}

fn apply_recent_windows(
    config: &mut Config,
    key: &str,
    value: &Value,
    errors: &mut Vec<String>,
    saw_mru_binds: &mut bool,
) {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return;
        }
    };

    let mut part = RecentWindowsPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "on" => part.on = read_flag(&value),
            "off" => part.off = read_flag(&value),
            "debounce_ms" => part.debounce_ms = read_u16(&value, "recent_windows.debounce_ms", errors),
            "open_delay_ms" => part.open_delay_ms = read_u16(&value, "recent_windows.open_delay_ms", errors),
            "highlight" => part.highlight = read_mru_highlight(&value, "recent_windows.highlight", errors),
            "previews" => part.previews = read_mru_previews(&value, "recent_windows.previews", errors),
            "binds" => part.binds = read_mru_binds(&value, "recent_windows.binds", errors),
            _ => push_err(errors, "recent_windows", format!("unexpected key `{key}`")),
        }
    }

    // When an MRU binds section is encountered for the first time, clear out the default MRU
    // binds.
    if !*saw_mru_binds && part.binds.is_some() {
        *saw_mru_binds = true;
        config.recent_windows.binds.clear();
    }

    config.recent_windows.merge_with(&part);
}

fn read_mru_highlight(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<MruHighlightPart> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };

    let mut part = MruHighlightPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "active_color" => part.active_color = read_color(&value, &format!("{key}.active_color"), errors),
            "urgent_color" => part.urgent_color = read_color(&value, &format!("{key}.urgent_color"), errors),
            "padding" => part.padding = read_float_or_int(&value, &format!("{key}.padding"), errors),
            "corner_radius" => part.corner_radius = read_float_or_int(&value, &format!("{key}.corner_radius"), errors),
            _ => push_err(errors, &key, format!("unexpected key `{key}`")),
        }
    }

    Some(part)
}

fn read_mru_previews(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<MruPreviewsPart> {
    let t = match value {
        Value::Table(t) => t,
        _ => {
            push_err(errors, key, format!("expected a table, got {}", type_name(value)));
            return None;
        }
    };

    let mut part = MruPreviewsPart::default();
    let it = t.pairs::<String, Value>();
    for kv in it {
        let (key, value) = match kv { Ok(kv) => kv, Err(_) => continue };
        let key = normalize_key(&key);
        match key.as_str() {
            "max_height" => part.max_height = read_float_or_int(&value, &format!("{key}.max_height"), errors),
            "max_scale" => part.max_scale = read_float_or_int(&value, &format!("{key}.max_scale"), errors),
            _ => push_err(errors, &key, format!("unexpected key `{key}`")),
        }
    }

    Some(part)
}

fn read_mru_binds(value: &Value, key: &str, errors: &mut Vec<String>) -> Option<MruBinds> {
    let items = read_list_of_tables(value, key, errors)?;

    let mut binds = Vec::new();
    for item in items {
        let Some(mru_bind) = read_mru_bind(&item, errors) else {
            continue;
        };
        binds.push(mru_bind);
    }

    Some(MruBinds(binds))
}

fn read_mru_bind(t: &Table, errors: &mut Vec<String>) -> Option<MruBind> {
    let key_str = match opts_raw_get(t, "key") {
        Value::String(s) => s.to_string_lossy(),
        _ => {
            push_err(errors, "recent_windows.binds", "missing `key`");
            return None;
        }
    };

    let key = match key_str.trim().parse::<Key>() {
        Ok(k) => k,
        Err(err) => {
            push_err(errors, "recent_windows.binds", format!("{err}"));
            return None;
        }
    };

    let action = match opts_raw_get(t, "action") {
        Value::Table(t) => read_mru_action(&t, errors)?,
        other => {
            push_err(
                errors,
                "recent_windows.binds",
                format!("action must be a table, got {}", type_name(&other)),
            );
            return None;
        }
    };

    let bind = MruBind {
        key,
        action,
        allow_inhibiting: read_flag_default(&opts_raw_get(t, "allow_inhibiting"), true),
        hotkey_overlay_title: read_hotkey_overlay_title(t, errors),
    };

    Some(bind)
}

fn read_mru_action(t: &Table, errors: &mut Vec<String>) -> Option<MruAction> {
    let name = read_required_string(t, "name", errors)?;

    let scope = read_mru_scope(t, errors);
    let filter = read_mru_filter(t, errors).unwrap_or(MruFilter::All);

    match normalize_key(&name).as_str() {
        "next_window" => Some(MruAction::NextWindow(scope, filter)),
        "previous_window" => Some(MruAction::PreviousWindow(scope, filter)),
        _ => {
            push_err(errors, "recent_windows.binds", format!("unknown action `{name}`"));
            None
        }
    }
}

fn apply_set_layout_defaults(ctx: &Arc<Mutex<ParseCtx>>, t: &Table) -> mlua::Result<()> {
    let mode = match opts_raw_get(t, "mode") {
        Value::String(s) => s.to_string_lossy(),
        other => {
            let mut g = ctx.lock().unwrap();
            g.record_validation(
                "set_layout_defaults",
                format!("expected a `mode`, got {}", type_name(&other)),
            );
            return Ok(());
        }
    };

    if !matches!(mode.as_str(), "dwindle") {
        let mut g = ctx.lock().unwrap();
        g.record_validation("set_layout_defaults", format!("unknown mode `{mode}`"));
        return Ok(());
    }

    let part = LayoutPart {
        default_column_display: Some(ColumnDisplay::Dwindle),
        preset_column_widths: Some(vec![
            PresetSize::Proportion(0.33333),
            PresetSize::Proportion(0.5),
            PresetSize::Proportion(0.66667),
        ]),
        default_column_width: Some(DefaultPresetSize(Some(PresetSize::Proportion(0.5)))),
        preset_window_heights: Some(vec![
            PresetSize::Proportion(0.33333),
            PresetSize::Proportion(0.5),
            PresetSize::Proportion(0.66667),
        ]),
        ..Default::default()
    };

    let mut g = ctx.lock().unwrap();
    g.config.layout.merge_with(&part);

    Ok(())
}