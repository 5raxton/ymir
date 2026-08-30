//! ymir config parsing.
//!
//! The config is a Lua program, evaluated in a fresh sandboxed VM (see the `lua` module). A small
//! prelude exposes the config API (`ymir.*`, `include_config`, tracked `dofile`/`require`) and
//! folds the resulting data table — or the imperative calls — into a `Config` through the `*Part`
//! types and their `MergeWith` impls.
//!
//! The config can be constructed from multiple files (includes). To support this, many types are
//! split into two. For example, `Layout` and `LayoutPart` where `Layout` is the final config and
//! `LayoutPart` is one part parsed from one config file.
//!
//! The convention for `Default` impls is to set the initial values before the parsing occurs.
//! Then, parsing will update the values with those parsed from the config.
//!
//! The `Default` values match those from `resources/default-config.lua` in almost all cases, with
//! a notable exception of `binds {}` and some window rules.

#[macro_use]
extern crate tracing;

use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use miette::{miette, Context as _, IntoDiagnostic as _};

#[macro_use]
pub mod macros;

pub mod animations;
pub mod appearance;
pub mod binds;
pub mod debug;
pub mod error;
pub mod gestures;
pub mod input;
pub mod layer_rule;
pub mod layout;
pub mod lua;
pub mod misc;
pub mod output;
pub mod recent_windows;
pub mod utils;
pub mod window_rule;
pub mod workspace;

pub use crate::animations::{Animation, Animations};
pub use crate::appearance::*;
pub use crate::binds::*;
pub use crate::debug::Debug;
pub use crate::error::{ConfigIncludeError, ConfigParseResult};
pub use crate::gestures::Gestures;
pub use crate::input::{Input, ModKey, ScrollMethod, TrackLayout, WarpMouseToFocusMode, Xkb};
pub use crate::layer_rule::LayerRule;
pub use crate::layout::*;
pub use crate::misc::*;
pub use crate::output::{Output, OutputName, Outputs, Position, Vrr};
pub use crate::recent_windows::{MruDirection, MruFilter, MruPreviews, MruScope, RecentWindows};
pub use crate::utils::FloatOrInt;
pub use crate::window_rule::{
    FloatingPosition, OnXdgActivate, PopupsRule, RelativeTo, ResolvedPopupsRules, WindowRule,
};
pub use crate::workspace::{Workspace, WorkspaceLayoutPart};

#[derive(Debug, Default, PartialEq)]
pub struct Config {
    pub input: Input,
    pub outputs: Outputs,
    pub spawn_at_startup: Vec<SpawnAtStartup>,
    pub spawn_sh_at_startup: Vec<SpawnShAtStartup>,
    pub layout: Layout,
    pub prefer_no_csd: bool,
    pub cursor: Cursor,
    pub screenshot_path: ScreenshotPath,
    pub clipboard: Clipboard,
    pub hotkey_overlay: HotkeyOverlay,
    pub config_notification: ConfigNotification,
    pub animations: Animations,
    pub blur: Blur,
    pub gestures: Gestures,
    pub overview: Overview,
    pub environment: Environment,
    pub xwayland_satellite: XwaylandSatellite,
    pub window_rules: Vec<WindowRule>,
    pub layer_rules: Vec<LayerRule>,
    pub binds: Binds,
    pub switch_events: SwitchBinds,
    pub debug: Debug,
    pub workspaces: Vec<Workspace>,
    pub recent_windows: RecentWindows,
}

#[derive(Debug, Clone)]
pub enum ConfigPath {
    /// Explicitly set config path.
    ///
    /// Load the config only from this path, never create it.
    Explicit(PathBuf),

    /// Default config path.
    ///
    /// Prioritize the user path, fallback to the system path, fallback to creating the user path
    /// at compositor startup.
    Regular {
        /// User config path, usually `$XDG_CONFIG_HOME/ymir/config.lua`.
        user_path: PathBuf,
        /// System config path, usually `/etc/ymir/config.lua`.
        system_path: PathBuf,
    },
}

// The `lua` module implements the Lua VM, its prelude, and the data-table/imperative config
// API. The `*Part` types in the section modules still provide all of the merge logic; the value
// returned by a config program is folded into a `Config` by the module's section appliers.

impl Config {
    pub fn load_default() -> ConfigParseResult<Self, miette::Report> {
        let res = Config::parse(
            Path::new("default-config.lua"),
            include_str!("../../resources/default-config.lua"),
        );

        let includes_in_default = !res.includes.is_empty();
        let config = match res.config {
            Ok(config) if !includes_in_default => Ok(config),
            // Includes in the default config would require files that only exist on disk at
            // runtime. Report this as a parse error instead of asserting, so a broken embedded
            // default is diagnosable rather than an unconditional boot panic.
            Ok(_) => Err(miette!(
                "the embedded default config unexpectedly has includes: {}",
                res.includes
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            )),
            Err(main) => Err(miette!(main).context("error parsing the embedded default config")),
        };

        ConfigParseResult {
            config,
            includes: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> ConfigParseResult<Self, miette::Report> {
        let contents = match fs::read_to_string(path) {
            Ok(x) => x,
            Err(err) => {
                return ConfigParseResult::from_err(
                    miette!(err).context(format!("error reading {path:?}")),
                );
            }
        };

        Self::parse(path, &contents).map_config_res(|res| {
            let config = res.context("error parsing")?;
            debug!("loaded config from {path:?}");
            Ok(config)
        })
    }

    pub fn parse(path: &Path, text: &str) -> ConfigParseResult<Self, ConfigIncludeError> {
        match lua::run_program(path, text) {
            Ok((config, includes)) => ConfigParseResult {
                config: Ok(config),
                includes,
            },
            Err((main, includes)) => ConfigParseResult {
                config: Err(main),
                includes,
            },
        }
    }

    pub fn parse_mem(text: &str) -> Result<Self, ConfigIncludeError> {
        Self::parse(Path::new("config.lua"), text).config
    }
}

impl ConfigPath {
    /// Loads the config, returns an error if it doesn't exist.
    pub fn load(&self) -> ConfigParseResult<Config, miette::Report> {
        let _span = tracy_client::span!("ConfigPath::load");

        self.load_inner(|user_path, system_path| {
            Err(miette!(
                "no config file found; create one at {user_path:?} or {system_path:?}",
            ))
        })
        .map_config_res(|res| res.context("error loading config"))
    }

    /// Loads the config, or creates it if it doesn't exist.
    ///
    /// Returns a tuple containing the path that was created, if any, and the loaded config.
    ///
    /// If the config was created, but for some reason could not be read afterwards,
    /// this may return `(Some(_), Err(_))`.
    pub fn load_or_create(&self) -> (Option<&Path>, ConfigParseResult<Config, miette::Report>) {
        let _span = tracy_client::span!("ConfigPath::load_or_create");

        let mut created_at = None;

        let result = self
            .load_inner(|user_path, _| {
                Self::create(user_path, &mut created_at)
                    .map(|()| user_path)
                    .with_context(|| format!("error creating config at {user_path:?}"))
            })
            .map_config_res(|res| res.context("error loading config"));

        (created_at, result)
    }

    fn load_inner<'a>(
        &'a self,
        maybe_create: impl FnOnce(&'a Path, &'a Path) -> miette::Result<&'a Path>,
    ) -> ConfigParseResult<Config, miette::Report> {
        let path = match self {
            ConfigPath::Explicit(path) => path.as_path(),
            ConfigPath::Regular {
                user_path,
                system_path,
            } => {
                if user_path.exists() {
                    user_path.as_path()
                } else if system_path.exists() {
                    system_path.as_path()
                } else {
                    match maybe_create(user_path.as_path(), system_path.as_path()) {
                        Ok(x) => x,
                        Err(err) => return ConfigParseResult::from_err(miette!(err)),
                    }
                }
            }
        };
        Config::load(path)
    }

    fn create<'a>(path: &'a Path, created_at: &mut Option<&'a Path>) -> miette::Result<()> {
        if let Some(default_parent) = path.parent() {
            fs::create_dir_all(default_parent)
                .into_diagnostic()
                .with_context(|| format!("error creating config directory {default_parent:?}"))?;
        }

        // Create the config and fill it with the default config if it doesn't exist.
        let mut new_file = match File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
        {
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
            res => res,
        }
        .into_diagnostic()
        .with_context(|| format!("error opening config file at {path:?}"))?;

        *created_at = Some(path);

        let default = include_bytes!("../../resources/default-config.lua");

        new_file
            .write_all(default)
            .into_diagnostic()
            .with_context(|| format!("error writing default config to {path:?}"))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use insta::{assert_debug_snapshot, assert_snapshot};
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn can_create_default_config() {
        let _ = Config::load_default()
            .config
            .expect("embedded default config must parse");
    }

    #[test]
    fn can_create_dwindle_config() {
        // The dwindle example config is a valid variant of the default config.
        let source = include_str!("../../resources/dwindle-config.lua");
        let config = Config::parse_mem(source).unwrap();
        assert_eq!(
            config.layout.default_column_display,
            ymir_ipc::ColumnDisplay::Dwindle,
        );
    }

    #[test]
    fn default_repeat_params() {
        let config = Config::parse_mem("").unwrap();
        assert_eq!(config.input.keyboard.repeat_delay, 600);
        assert_eq!(config.input.keyboard.repeat_rate, 25);
    }

    #[track_caller]
    fn do_parse(text: &str) -> Config {
        Config::parse_mem(text)
            .map_err(miette::Report::new)
            .unwrap()
    }

    #[test]
    fn parse_on_xdg_activate() {
        let parsed = do_parse(
            r#"
            return {
                window_rules = {
                    { on_xdg_activate = "ignore" },
                    { on_xdg_activate = "set-urgent" },
                    { on_xdg_activate = "focus" },
                },
            }
            "#,
        );

        assert_eq!(
            parsed
                .window_rules
                .iter()
                .map(|rule| rule.on_xdg_activate)
                .collect::<Vec<_>>(),
            vec![
                Some(OnXdgActivate::Ignore),
                Some(OnXdgActivate::SetUrgent),
                Some(OnXdgActivate::Focus),
            ]
        );
    }

    #[test]
    fn parse() {
        let parsed = do_parse(
            r##"
            return {
                input = {
                    keyboard = {
                        repeat_delay = 600,
                        repeat_rate = 25,
                        track_layout = "window",
                        xkb = {
                            layout = "us,ru",
                            options = "grp:win_space_toggle",
                        },
                    },

                    touchpad = {
                        tap = true,
                        dwt = true,
                        dwtp = true,
                        drag = true,
                        click_method = "clickfinger",
                        accel_speed = 0.2,
                        accel_profile = "flat",
                        scroll_method = "two-finger",
                        scroll_button = 272,
                        scroll_button_lock = true,
                        tap_button_map = "left-middle-right",
                        disabled_on_external_mouse = true,
                        scroll_factor = 0.9,
                    },

                    mouse = {
                        natural_scroll = true,
                        accel_speed = 0.4,
                        accel_profile = "flat",
                        scroll_method = "no-scroll",
                        scroll_button = 273,
                        middle_emulation = true,
                        scroll_factor = 0.2,
                    },

                    trackpoint = {
                        off = true,
                        natural_scroll = true,
                        accel_speed = 0.0,
                        accel_profile = "flat",
                        scroll_method = "on-button-down",
                        scroll_button = 274,
                    },

                    trackball = {
                        off = true,
                        natural_scroll = true,
                        accel_speed = 0.0,
                        accel_profile = "flat",
                        scroll_method = "edge",
                        scroll_button = 275,
                        scroll_button_lock = true,
                        left_handed = true,
                        middle_emulation = true,
                    },

                    tablet = {
                        map_to_output = "eDP-1",
                        map_to_focused_output = true,
                        map_to_focused_window = true,
                        calibration_matrix = { 1.0, 2.0, 3.0, 4.0, 5.0, 6.0 },
                    },

                    touch = {
                        map_to_output = "eDP-1",
                    },

                    disable_power_key_handling = true,
                    warp_mouse_to_focus = true,
                    focus_follows_mouse = true,
                    workspace_auto_back_and_forth = true,

                    mod_key = "Mod5",
                    mod_key_nested = "Super",
                },

                output = {
                    {
                        name = "eDP-1",
                        focus_at_startup = true,
                        scale = 2,
                        transform = "flipped-90",
                        position = { x = 10, y = 20 },
                        mode = "1920x1080@144",
                        max_bpc = "10",
                        variable_refresh_rate = { on_demand = true },
                        background_color = "rgba(25, 25, 102, 1.0)",
                        hot_corners = {
                            off = true,
                            top_left = true,
                            top_right = true,
                            bottom_left = true,
                            bottom_right = true,
                        },
                    },
                    {
                        name = "eDP-2",
                        mode = { mode = "1920x1080@144", custom = true },
                    },
                    {
                        name = "eDP-3",
                        modeline = {
                            clock = 173.0,
                            hdisplay = 1920,
                            hsync_start = 2048,
                            hsync_end = 2248,
                            htotal = 2576,
                            vdisplay = 1080,
                            vsync_start = 1083,
                            vsync_end = 1088,
                            vtotal = 1120,
                            hsync_polarity = "-hsync",
                            vsync_polarity = "+vsync",
                        },
                    },
                },

                layout = {
                    focus_ring = {
                        width = 5,
                        active_color = { r = 0, g = 100, b = 200, a = 255 },
                        inactive_color = { r = 255, g = 200, b = 100, a = 0 },
                        active_gradient = {
                            from = "rgba(10, 20, 30, 1.0)",
                            to = "#0080ffff",
                            relative_to = "workspace-view",
                        },
                    },

                    border = {
                        width = 3,
                        inactive_color = "rgba(255, 200, 100, 0.0)",
                    },

                    shadow = {
                        offset = { x = 10, y = -20 },
                    },

                    tab_indicator = {
                        width = 10,
                        position = "top",
                    },

                    preset_column_widths = {
                        { proportion = 0.25 },
                        { proportion = 0.5 },
                        { fixed = 960 },
                        { fixed = 1280 },
                    },

                    preset_window_heights = {
                        { proportion = 0.25 },
                        { proportion = 0.5 },
                        { fixed = 960 },
                        { fixed = 1280 },
                    },

                    default_column_width = { proportion = 0.25 },

                    gaps = 8,

                    struts = { left = 1, right = 2, top = 3 },

                    center_focused_column = "on-overflow",

                    default_column_display = "tabbed",

                    insert_hint = {
                        color = "rgb(255, 200, 127)",
                        gradient = {
                            from = "rgba(10, 20, 30, 1.0)",
                            to = "#0080ffff",
                            relative_to = "workspace-view",
                        },
                    },
                },

                spawn_at_startup = {
                    { command = { "alacritty", "-e", "fish" } },
                },

                spawn_sh_at_startup = {
                    { command = "qs -c ~/source/qs/MyAwesomeShell" },
                },

                prefer_no_csd = true,

                cursor = {
                    xcursor_theme = "breeze_cursors",
                    xcursor_size = 16,
                    hide_when_typing = true,
                    hide_after_inactive_ms = 3000,
                },

                screenshot_path = "~/Screenshots/screenshot.png",

                clipboard = {
                    disable_primary = true,
                },

                hotkey_overlay = {
                    skip_at_startup = true,
                },

                animations = {
                    slowdown = 2.0,

                    workspace_switch = {
                        spring = { damping_ratio = 1.0, stiffness = 1000, epsilon = 0.0001 },
                    },

                    horizontal_view_movement = {
                        easing = { duration_ms = 100, curve = "ease-out-expo" },
                    },

                    window_open = {
                        off = true,
                        easing = { duration_ms = 150, curve = "ease-out-expo" },
                    },

                    window_close = {
                        easing = { duration_ms = 150, curve = "cubic-bezier(0.05, 0.7, 0.1, 1)" },
                    },

                    recent_windows_close = {
                        off = true,
                        spring = { damping_ratio = 1, stiffness = 800, epsilon = 0.001 },
                    },
                },

                gestures = {
                    dnd_edge_view_scroll = {
                        trigger_width = 10,
                        max_speed = 50,
                    },
                },

                environment = {
                    { name = "QT_QPA_PLATFORM", value = "wayland" },
                    { name = "DISPLAY", value = ymir.null },
                },

                window_rules = {
                    {
                        match = {
                            { app_id = ".*alacritty" },
                        },
                        exclude = {
                            { title = "~" },
                            { is_active = true, is_focused = false },
                        },

                        open_on_output = "eDP-1",
                        open_maximized = true,
                        open_fullscreen = false,
                        open_floating = false,
                        open_focused = true,
                        default_window_height = { fixed = 500 },
                        default_column_display = "tabbed",
                        default_floating_position = { x = 100, y = -200, relative_to = "bottom-left" },
                        on_xdg_activate = "ignore",

                        focus_ring = {
                            off = true,
                            width = 3,
                        },

                        border = {
                            on = true,
                            width = 8.5,
                        },

                        tab_indicator = {
                            active_color = "#f00",
                        },
                    },
                },

                layer_rules = {
                    {
                        match = { { namespace = "^notifications$" } },
                        block_out_from = "screencast",
                    },
                },

                binds = {
                    { key = "Mod+Escape", allow_inhibiting = false, hotkey_overlay_title = "Inhibit", action = { name = "toggle_keyboard_shortcuts_inhibit" } },
                    { key = "Mod+Shift+Escape", allow_inhibiting = false, action = { name = "toggle_keyboard_shortcuts_inhibit" } },
                    { key = "Mod+T", allow_when_locked = true, action = { name = "spawn", command = { "alacritty" } } },
                    { key = "Mod+Q", hotkey_overlay_title = false, action = { name = "close_window" } },
                    { key = "Mod+Shift+H", action = { name = "focus_monitor_left" } },
                    { key = "Mod+Shift+O", action = { name = "focus_monitor", output = "eDP-1" } },
                    { key = "Mod+Ctrl+Shift+L", action = { name = "move_window_to_monitor_right" } },
                    { key = "Mod+Ctrl+Alt+O", action = { name = "move_window_to_monitor", output = "eDP-1" } },
                    { key = "Mod+Ctrl+Alt+P", action = { name = "move_column_to_monitor", output = "DP-1" } },
                    { key = "Mod+Comma", action = { name = "consume_window_into_column" } },
                    { key = "Mod+1", action = { name = "focus_workspace", index = 1 } },
                    { key = "Mod+Shift+1", action = { name = "focus_workspace", workspace = "workspace-1" } },
                    { key = "Mod+Shift+E", allow_inhibiting = false, action = { name = "quit", skip_confirmation = true } },
                    { key = "Mod+WheelScrollDown", cooldown_ms = 150, action = { name = "focus_workspace_down" } },
                    { key = "Super+Alt+S", allow_when_locked = true, action = { name = "spawn_sh", command = "pkill orca || exec orca" } },
                },

                switch_events = {
                    tablet_mode_on = {
                        spawn = {
                            "bash",
                            "-c",
                            "gsettings set org.gnome.desktop.a11y.applications screen-keyboard-enabled true",
                        },
                    },
                    tablet_mode_off = {
                        spawn = {
                            "bash",
                            "-c",
                            "gsettings set org.gnome.desktop.a11y.applications screen-keyboard-enabled false",
                        },
                    },
                },

                debug = {
                    render_drm_device = "/dev/dri/renderD129",
                    ignored_drm_devices = { "/dev/dri/renderD128", "/dev/dri/renderD130" },
                },

                workspaces = {
                    {
                        name = "workspace-1",
                        open_on_output = "eDP-1",
                    },
                    { name = "workspace-2" },
                    { name = "workspace-3" },
                },

                recent_windows = {
                    off = true,

                    highlight = {
                        padding = 15,
                        active_color = "#00ff00",
                    },

                    previews = {
                        max_height = 960,
                    },

                    binds = {
                        { key = "Alt+Tab", action = { name = "next_window" } },
                        { key = "Alt+grave", action = { name = "next_window", filter = "app-id" } },
                        { key = "Super+Tab", action = { name = "next_window", scope = "output" } },
                    },
                },
            }
            "##,
        );

        assert_debug_snapshot!(parsed, @r#"
        Config {
            input: Input {
                keyboard: Keyboard {
                    xkb: Xkb {
                        rules: "",
                        model: "",
                        layout: "us,ru",
                        variant: "",
                        options: Some(
                            "grp:win_space_toggle",
                        ),
                        file: None,
                    },
                    repeat_delay: 600,
                    repeat_rate: 25,
                    track_layout: Window,
                    numlock: false,
                },
                touchpad: Touchpad {
                    off: false,
                    tap: true,
                    dwt: true,
                    dwtp: true,
                    drag: Some(
                        true,
                    ),
                    drag_lock: false,
                    natural_scroll: false,
                    click_method: Some(
                        Clickfinger,
                    ),
                    accel_speed: FloatOrInt(
                        0.2,
                    ),
                    accel_profile: Some(
                        Flat,
                    ),
                    scroll_method: Some(
                        TwoFinger,
                    ),
                    scroll_button: Some(
                        272,
                    ),
                    scroll_button_lock: true,
                    tap_button_map: Some(
                        LeftMiddleRight,
                    ),
                    left_handed: false,
                    disabled_on_external_mouse: true,
                    middle_emulation: false,
                    scroll_factor: Some(
                        ScrollFactor {
                            base: Some(
                                FloatOrInt(
                                    0.9,
                                ),
                            ),
                            horizontal: None,
                            vertical: None,
                        },
                    ),
                },
                mouse: Mouse {
                    off: false,
                    natural_scroll: true,
                    accel_speed: FloatOrInt(
                        0.4,
                    ),
                    accel_profile: Some(
                        Flat,
                    ),
                    scroll_method: Some(
                        NoScroll,
                    ),
                    scroll_button: Some(
                        273,
                    ),
                    scroll_button_lock: false,
                    left_handed: false,
                    middle_emulation: true,
                    scroll_factor: Some(
                        ScrollFactor {
                            base: Some(
                                FloatOrInt(
                                    0.2,
                                ),
                            ),
                            horizontal: None,
                            vertical: None,
                        },
                    ),
                },
                trackpoint: Trackpoint {
                    off: true,
                    natural_scroll: true,
                    accel_speed: FloatOrInt(
                        0.0,
                    ),
                    accel_profile: Some(
                        Flat,
                    ),
                    scroll_method: Some(
                        OnButtonDown,
                    ),
                    scroll_button: Some(
                        274,
                    ),
                    scroll_button_lock: false,
                    left_handed: false,
                    middle_emulation: false,
                },
                trackball: Trackball {
                    off: true,
                    natural_scroll: true,
                    accel_speed: FloatOrInt(
                        0.0,
                    ),
                    accel_profile: Some(
                        Flat,
                    ),
                    scroll_method: Some(
                        Edge,
                    ),
                    scroll_button: Some(
                        275,
                    ),
                    scroll_button_lock: true,
                    left_handed: true,
                    middle_emulation: true,
                },
                tablet: Tablet {
                    off: false,
                    calibration_matrix: Some(
                        [
                            1.0,
                            2.0,
                            3.0,
                            4.0,
                            5.0,
                            6.0,
                        ],
                    ),
                    map_to_output: Some(
                        "eDP-1",
                    ),
                    map_to_focused_output: true,
                    map_to_focused_window: true,
                    left_handed: false,
                },
                touch: Touch {
                    off: false,
                    calibration_matrix: None,
                    map_to_output: Some(
                        "eDP-1",
                    ),
                },
                disable_power_key_handling: true,
                warp_mouse_to_focus: Some(
                    WarpMouseToFocus {
                        mode: None,
                    },
                ),
                focus_follows_mouse: Some(
                    FocusFollowsMouse {
                        max_scroll_amount: None,
                    },
                ),
                workspace_auto_back_and_forth: true,
                mod_key: Some(
                    IsoLevel3Shift,
                ),
                mod_key_nested: Some(
                    Super,
                ),
            },
            outputs: Outputs(
                [
                    Output {
                        off: false,
                        name: "eDP-1",
                        scale: Some(
                            FloatOrInt(
                                2.0,
                            ),
                        ),
                        transform: Flipped90,
                        position: Some(
                            Position {
                                x: 10,
                                y: 20,
                            },
                        ),
                        max_bpc: Some(
                            MaxBpc(
                                _10,
                            ),
                        ),
                        mode: Some(
                            Mode {
                                custom: false,
                                mode: ConfiguredMode {
                                    width: 1920,
                                    height: 1080,
                                    refresh: Some(
                                        144.0,
                                    ),
                                },
                            },
                        ),
                        modeline: None,
                        variable_refresh_rate: Some(
                            Vrr {
                                on_demand: true,
                            },
                        ),
                        focus_at_startup: true,
                        background_color: Some(
                            Color {
                                r: 0.09803922,
                                g: 0.09803922,
                                b: 0.4,
                                a: 1.0,
                            },
                        ),
                        backdrop_color: None,
                        hot_corners: Some(
                            HotCorners {
                                off: true,
                                top_left: true,
                                top_right: true,
                                bottom_left: true,
                                bottom_right: true,
                            },
                        ),
                        layout: None,
                    },
                    Output {
                        off: false,
                        name: "eDP-2",
                        scale: None,
                        transform: Normal,
                        position: None,
                        max_bpc: None,
                        mode: Some(
                            Mode {
                                custom: true,
                                mode: ConfiguredMode {
                                    width: 1920,
                                    height: 1080,
                                    refresh: Some(
                                        144.0,
                                    ),
                                },
                            },
                        ),
                        modeline: None,
                        variable_refresh_rate: None,
                        focus_at_startup: false,
                        background_color: None,
                        backdrop_color: None,
                        hot_corners: None,
                        layout: None,
                    },
                    Output {
                        off: false,
                        name: "eDP-3",
                        scale: None,
                        transform: Normal,
                        position: None,
                        max_bpc: None,
                        mode: None,
                        modeline: Some(
                            Modeline {
                                clock: 173.0,
                                hdisplay: 1920,
                                hsync_start: 2048,
                                hsync_end: 2248,
                                htotal: 2576,
                                vdisplay: 1080,
                                vsync_start: 1083,
                                vsync_end: 1088,
                                vtotal: 1120,
                                hsync_polarity: NHSync,
                                vsync_polarity: PVSync,
                            },
                        ),
                        variable_refresh_rate: None,
                        focus_at_startup: false,
                        background_color: None,
                        backdrop_color: None,
                        hot_corners: None,
                        layout: None,
                    },
                ],
            ),
            spawn_at_startup: [
                SpawnAtStartup {
                    command: [
                        "alacritty",
                        "-e",
                        "fish",
                    ],
                },
            ],
            spawn_sh_at_startup: [
                SpawnShAtStartup {
                    command: "qs -c ~/source/qs/MyAwesomeShell",
                },
            ],
            layout: Layout {
                focus_ring: FocusRing {
                    off: false,
                    width: 5.0,
                    active_color: Color {
                        r: 0.0,
                        g: 0.39215687,
                        b: 0.78431374,
                        a: 1.0,
                    },
                    inactive_color: Color {
                        r: 1.0,
                        g: 0.78431374,
                        b: 0.39215687,
                        a: 0.0,
                    },
                    urgent_color: Color {
                        r: 0.60784316,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                    active_gradient: Some(
                        Gradient {
                            from: Color {
                                r: 0.039215688,
                                g: 0.078431375,
                                b: 0.11764706,
                                a: 1.0,
                            },
                            to: Color {
                                r: 0.0,
                                g: 0.5019608,
                                b: 1.0,
                                a: 1.0,
                            },
                            angle: 180,
                            relative_to: WorkspaceView,
                            in_: GradientInterpolation {
                                color_space: Srgb,
                                hue_interpolation: Shorter,
                            },
                        },
                    ),
                    inactive_gradient: None,
                    urgent_gradient: None,
                },
                border: Border {
                    off: false,
                    width: 3.0,
                    active_color: Color {
                        r: 1.0,
                        g: 0.78431374,
                        b: 0.49803922,
                        a: 1.0,
                    },
                    inactive_color: Color {
                        r: 1.0,
                        g: 0.78431374,
                        b: 0.39215687,
                        a: 0.0,
                    },
                    urgent_color: Color {
                        r: 0.60784316,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                    active_gradient: None,
                    inactive_gradient: None,
                    urgent_gradient: None,
                },
                shadow: Shadow {
                    on: false,
                    offset: ShadowOffset {
                        x: FloatOrInt(
                            10.0,
                        ),
                        y: FloatOrInt(
                            -20.0,
                        ),
                    },
                    softness: 30.0,
                    spread: 5.0,
                    draw_behind_window: false,
                    color: Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.46666667,
                    },
                    inactive_color: None,
                },
                tab_indicator: TabIndicator {
                    off: false,
                    hide_when_single_tab: false,
                    place_within_column: false,
                    gap: 5.0,
                    width: 10.0,
                    length: TabIndicatorLength {
                        total_proportion: Some(
                            0.5,
                        ),
                    },
                    position: Top,
                    gaps_between_tabs: 0.0,
                    corner_radius: 0.0,
                    active_color: None,
                    inactive_color: None,
                    urgent_color: None,
                    active_gradient: None,
                    inactive_gradient: None,
                    urgent_gradient: None,
                },
                insert_hint: InsertHint {
                    off: false,
                    color: Color {
                        r: 1.0,
                        g: 0.78431374,
                        b: 0.49803922,
                        a: 1.0,
                    },
                    gradient: Some(
                        Gradient {
                            from: Color {
                                r: 0.039215688,
                                g: 0.078431375,
                                b: 0.11764706,
                                a: 1.0,
                            },
                            to: Color {
                                r: 0.0,
                                g: 0.5019608,
                                b: 1.0,
                                a: 1.0,
                            },
                            angle: 180,
                            relative_to: WorkspaceView,
                            in_: GradientInterpolation {
                                color_space: Srgb,
                                hue_interpolation: Shorter,
                            },
                        },
                    ),
                },
                preset_column_widths: [
                    Proportion(
                        0.25,
                    ),
                    Proportion(
                        0.5,
                    ),
                    Fixed(
                        960,
                    ),
                    Fixed(
                        1280,
                    ),
                ],
                default_column_width: Some(
                    Proportion(
                        0.25,
                    ),
                ),
                preset_window_heights: [
                    Proportion(
                        0.25,
                    ),
                    Proportion(
                        0.5,
                    ),
                    Fixed(
                        960,
                    ),
                    Fixed(
                        1280,
                    ),
                ],
                center_focused_column: OnOverflow,
                always_center_single_column: false,
                empty_workspace_above_first: false,
                default_column_display: Tabbed,
                gaps: 8.0,
                struts: Struts {
                    left: FloatOrInt(
                        1.0,
                    ),
                    right: FloatOrInt(
                        2.0,
                    ),
                    top: FloatOrInt(
                        3.0,
                    ),
                    bottom: FloatOrInt(
                        0.0,
                    ),
                },
                background_color: Color {
                    r: 0.25,
                    g: 0.25,
                    b: 0.25,
                    a: 1.0,
                },
                depth_queue: DepthQueue {
                    card_height_ratio: 0.62,
                    top_deck_size: 2,
                    bottom_deck_size: 2,
                    gap: 12.0,
                    deck_bleed: 24.0,
                    min_opacity: 0.35,
                    blur_radius: 18.0,
                    card_shadow: DepthDeckShadow {
                        on: true,
                        offset: ShadowOffset {
                            x: FloatOrInt(
                                0.0,
                            ),
                            y: FloatOrInt(
                                10.0,
                            ),
                        },
                        blur: 24.0,
                        color: Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.27058825,
                        },
                    },
                    perspective_tilt: 7.0,
                    focus_shuffle: SpringParams {
                        damping_ratio: 0.62,
                        stiffness: 750,
                        epsilon: 0.0001,
                    },
                },
            },
            prefer_no_csd: true,
            cursor: Cursor {
                xcursor_theme: "breeze_cursors",
                xcursor_size: 16,
                hide_when_typing: true,
                hide_after_inactive_ms: Some(
                    3000,
                ),
            },
            screenshot_path: ScreenshotPath(
                Some(
                    "~/Screenshots/screenshot.png",
                ),
            ),
            clipboard: Clipboard {
                disable_primary: true,
            },
            hotkey_overlay: HotkeyOverlay {
                skip_at_startup: true,
                hide_not_bound: false,
            },
            config_notification: ConfigNotification {
                disable_failed: false,
            },
            animations: Animations {
                off: false,
                slowdown: 2.0,
                workspace_switch: WorkspaceSwitchAnim(
                    Animation {
                        off: false,
                        kind: Spring(
                            SpringParams {
                                damping_ratio: 1.0,
                                stiffness: 1000,
                                epsilon: 0.0001,
                            },
                        ),
                    },
                ),
                window_open: WindowOpenAnim {
                    anim: Animation {
                        off: true,
                        kind: Easing(
                            EasingParams {
                                duration_ms: 150,
                                curve: EaseOutExpo,
                            },
                        ),
                    },
                    custom_shader: None,
                },
                window_close: WindowCloseAnim {
                    anim: Animation {
                        off: false,
                        kind: Easing(
                            EasingParams {
                                duration_ms: 150,
                                curve: CubicBezier(
                                    0.05,
                                    0.7,
                                    0.1,
                                    1.0,
                                ),
                            },
                        ),
                    },
                    custom_shader: None,
                },
                horizontal_view_movement: HorizontalViewMovementAnim(
                    Animation {
                        off: false,
                        kind: Easing(
                            EasingParams {
                                duration_ms: 100,
                                curve: EaseOutExpo,
                            },
                        ),
                    },
                ),
                window_movement: WindowMovementAnim(
                    Animation {
                        off: false,
                        kind: Spring(
                            SpringParams {
                                damping_ratio: 1.0,
                                stiffness: 800,
                                epsilon: 0.0001,
                            },
                        ),
                    },
                ),
                window_resize: WindowResizeAnim {
                    anim: Animation {
                        off: false,
                        kind: Spring(
                            SpringParams {
                                damping_ratio: 1.0,
                                stiffness: 800,
                                epsilon: 0.0001,
                            },
                        ),
                    },
                    custom_shader: None,
                },
                config_notification_open_close: ConfigNotificationOpenCloseAnim(
                    Animation {
                        off: false,
                        kind: Spring(
                            SpringParams {
                                damping_ratio: 0.6,
                                stiffness: 1000,
                                epsilon: 0.001,
                            },
                        ),
                    },
                ),
                exit_confirmation_open_close: ExitConfirmationOpenCloseAnim(
                    Animation {
                        off: false,
                        kind: Spring(
                            SpringParams {
                                damping_ratio: 0.6,
                                stiffness: 500,
                                epsilon: 0.01,
                            },
                        ),
                    },
                ),
                screenshot_ui_open: ScreenshotUiOpenAnim(
                    Animation {
                        off: false,
                        kind: Easing(
                            EasingParams {
                                duration_ms: 200,
                                curve: EaseOutQuad,
                            },
                        ),
                    },
                ),
                overview_open_close: OverviewOpenCloseAnim(
                    Animation {
                        off: false,
                        kind: Spring(
                            SpringParams {
                                damping_ratio: 1.0,
                                stiffness: 800,
                                epsilon: 0.0001,
                            },
                        ),
                    },
                ),
                recent_windows_close: RecentWindowsCloseAnim(
                    Animation {
                        off: true,
                        kind: Spring(
                            SpringParams {
                                damping_ratio: 1.0,
                                stiffness: 800,
                                epsilon: 0.001,
                            },
                        ),
                    },
                ),
            },
            blur: Blur {
                off: false,
                passes: 3,
                offset: 3.0,
                noise: 0.02,
                saturation: 1.5,
            },
            gestures: Gestures {
                dnd_edge_view_scroll: DndEdgeViewScroll {
                    trigger_width: 10.0,
                    delay_ms: 100,
                    max_speed: 50.0,
                },
                dnd_edge_workspace_switch: DndEdgeWorkspaceSwitch {
                    trigger_height: 50.0,
                    delay_ms: 100,
                    max_speed: 1500.0,
                },
                hot_corners: HotCorners {
                    off: false,
                    top_left: false,
                    top_right: false,
                    bottom_left: false,
                    bottom_right: false,
                },
            },
            overview: Overview {
                zoom: 0.5,
                backdrop_color: Color {
                    r: 0.15,
                    g: 0.15,
                    b: 0.15,
                    a: 1.0,
                },
                workspace_shadow: WorkspaceShadow {
                    off: false,
                    offset: ShadowOffset {
                        x: FloatOrInt(
                            0.0,
                        ),
                        y: FloatOrInt(
                            10.0,
                        ),
                    },
                    softness: 40.0,
                    spread: 10.0,
                    color: Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.3137255,
                    },
                },
            },
            environment: Environment(
                [
                    EnvironmentVariable {
                        name: "QT_QPA_PLATFORM",
                        value: Some(
                            "wayland",
                        ),
                    },
                    EnvironmentVariable {
                        name: "DISPLAY",
                        value: None,
                    },
                ],
            ),
            xwayland_satellite: XwaylandSatellite {
                off: false,
                path: "xwayland-satellite",
            },
            window_rules: [
                WindowRule {
                    matches: [
                        Match {
                            app_id: Some(
                                RegexEq(
                                    Regex(
                                        ".*alacritty",
                                    ),
                                ),
                            ),
                            title: None,
                            is_active: None,
                            is_focused: None,
                            is_active_in_column: None,
                            is_floating: None,
                            is_window_cast_target: None,
                            is_urgent: None,
                            at_startup: None,
                        },
                    ],
                    excludes: [
                        Match {
                            app_id: None,
                            title: Some(
                                RegexEq(
                                    Regex(
                                        "~",
                                    ),
                                ),
                            ),
                            is_active: None,
                            is_focused: None,
                            is_active_in_column: None,
                            is_floating: None,
                            is_window_cast_target: None,
                            is_urgent: None,
                            at_startup: None,
                        },
                        Match {
                            app_id: None,
                            title: None,
                            is_active: Some(
                                true,
                            ),
                            is_focused: Some(
                                false,
                            ),
                            is_active_in_column: None,
                            is_floating: None,
                            is_window_cast_target: None,
                            is_urgent: None,
                            at_startup: None,
                        },
                    ],
                    default_column_width: None,
                    default_window_height: Some(
                        DefaultPresetSize(
                            Some(
                                Fixed(
                                    500,
                                ),
                            ),
                        ),
                    ),
                    open_on_output: Some(
                        "eDP-1",
                    ),
                    open_on_workspace: None,
                    open_maximized: Some(
                        true,
                    ),
                    open_maximized_to_edges: None,
                    open_fullscreen: Some(
                        false,
                    ),
                    open_floating: Some(
                        false,
                    ),
                    open_focused: Some(
                        true,
                    ),
                    on_xdg_activate: Some(
                        Ignore,
                    ),
                    min_width: None,
                    min_height: None,
                    max_width: None,
                    max_height: None,
                    focus_ring: BorderRule {
                        off: true,
                        on: false,
                        width: Some(
                            FloatOrInt(
                                3.0,
                            ),
                        ),
                        active_color: None,
                        inactive_color: None,
                        urgent_color: None,
                        active_gradient: None,
                        inactive_gradient: None,
                        urgent_gradient: None,
                    },
                    border: BorderRule {
                        off: false,
                        on: true,
                        width: Some(
                            FloatOrInt(
                                8.5,
                            ),
                        ),
                        active_color: None,
                        inactive_color: None,
                        urgent_color: None,
                        active_gradient: None,
                        inactive_gradient: None,
                        urgent_gradient: None,
                    },
                    shadow: ShadowRule {
                        off: false,
                        on: false,
                        offset: None,
                        softness: None,
                        spread: None,
                        draw_behind_window: None,
                        color: None,
                        inactive_color: None,
                    },
                    tab_indicator: TabIndicatorRule {
                        active_color: Some(
                            Color {
                                r: 1.0,
                                g: 0.0,
                                b: 0.0,
                                a: 1.0,
                            },
                        ),
                        inactive_color: None,
                        urgent_color: None,
                        active_gradient: None,
                        inactive_gradient: None,
                        urgent_gradient: None,
                    },
                    draw_border_with_background: None,
                    opacity: None,
                    geometry_corner_radius: None,
                    clip_to_geometry: None,
                    baba_is_float: None,
                    block_out_from: None,
                    variable_refresh_rate: None,
                    default_column_display: Some(
                        Tabbed,
                    ),
                    default_floating_position: Some(
                        FloatingPosition {
                            x: FloatOrInt(
                                100.0,
                            ),
                            y: FloatOrInt(
                                -200.0,
                            ),
                            relative_to: BottomLeft,
                        },
                    ),
                    scroll_factor: None,
                    tiled_state: None,
                    background_effect: BackgroundEffectRule {
                        xray: None,
                        blur: None,
                        noise: None,
                        saturation: None,
                    },
                    popups: PopupsRule {
                        opacity: None,
                        geometry_corner_radius: None,
                        background_effect: BackgroundEffectRule {
                            xray: None,
                            blur: None,
                            noise: None,
                            saturation: None,
                        },
                    },
                },
            ],
            layer_rules: [
                LayerRule {
                    matches: [
                        Match {
                            namespace: Some(
                                RegexEq(
                                    Regex(
                                        "^notifications$",
                                    ),
                                ),
                            ),
                            at_startup: None,
                            layer: None,
                        },
                    ],
                    excludes: [],
                    opacity: None,
                    block_out_from: Some(
                        Screencast,
                    ),
                    shadow: ShadowRule {
                        off: false,
                        on: false,
                        offset: None,
                        softness: None,
                        spread: None,
                        draw_behind_window: None,
                        color: None,
                        inactive_color: None,
                    },
                    geometry_corner_radius: None,
                    place_within_backdrop: None,
                    baba_is_float: None,
                    background_effect: BackgroundEffectRule {
                        xray: None,
                        blur: None,
                        noise: None,
                        saturation: None,
                    },
                    popups: PopupsRule {
                        opacity: None,
                        geometry_corner_radius: None,
                        background_effect: BackgroundEffectRule {
                            xray: None,
                            blur: None,
                            noise: None,
                            saturation: None,
                        },
                    },
                },
            ],
            binds: Binds(
                [
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_Escape,
                            ),
                            modifiers: Modifiers(
                                COMPOSITOR,
                            ),
                        },
                        action: ToggleKeyboardShortcutsInhibit,
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: false,
                        allow_inhibiting: false,
                        hotkey_overlay_title: Some(
                            Some(
                                "Inhibit",
                            ),
                        ),
                    },
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_Escape,
                            ),
                            modifiers: Modifiers(
                                SHIFT | COMPOSITOR,
                            ),
                        },
                        action: ToggleKeyboardShortcutsInhibit,
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: false,
                        allow_inhibiting: false,
                        hotkey_overlay_title: None,
                    },
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_t,
                            ),
                            modifiers: Modifiers(
                                COMPOSITOR,
                            ),
                        },
                        action: Spawn(
                            [
                                "alacritty",
                            ],
                        ),
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: true,
                        allow_inhibiting: true,
                        hotkey_overlay_title: None,
                    },
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_q,
                            ),
                            modifiers: Modifiers(
                                COMPOSITOR,
                            ),
                        },
                        action: CloseWindow,
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: false,
                        allow_inhibiting: true,
                        hotkey_overlay_title: Some(
                            None,
                        ),
                    },
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_h,
                            ),
                            modifiers: Modifiers(
                                SHIFT | COMPOSITOR,
                            ),
                        },
                        action: FocusMonitorLeft,
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: false,
                        allow_inhibiting: true,
                        hotkey_overlay_title: None,
                    },
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_o,
                            ),
                            modifiers: Modifiers(
                                SHIFT | COMPOSITOR,
                            ),
                        },
                        action: FocusMonitor(
                            "eDP-1",
                        ),
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: false,
                        allow_inhibiting: true,
                        hotkey_overlay_title: None,
                    },
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_l,
                            ),
                            modifiers: Modifiers(
                                CTRL | SHIFT | COMPOSITOR,
                            ),
                        },
                        action: MoveWindowToMonitorRight,
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: false,
                        allow_inhibiting: true,
                        hotkey_overlay_title: None,
                    },
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_o,
                            ),
                            modifiers: Modifiers(
                                CTRL | ALT | COMPOSITOR,
                            ),
                        },
                        action: MoveWindowToMonitor(
                            "eDP-1",
                        ),
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: false,
                        allow_inhibiting: true,
                        hotkey_overlay_title: None,
                    },
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_p,
                            ),
                            modifiers: Modifiers(
                                CTRL | ALT | COMPOSITOR,
                            ),
                        },
                        action: MoveColumnToMonitor(
                            "DP-1",
                        ),
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: false,
                        allow_inhibiting: true,
                        hotkey_overlay_title: None,
                    },
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_comma,
                            ),
                            modifiers: Modifiers(
                                COMPOSITOR,
                            ),
                        },
                        action: ConsumeWindowIntoColumn,
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: false,
                        allow_inhibiting: true,
                        hotkey_overlay_title: None,
                    },
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_1,
                            ),
                            modifiers: Modifiers(
                                COMPOSITOR,
                            ),
                        },
                        action: FocusWorkspace(
                            Index(
                                1,
                            ),
                        ),
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: false,
                        allow_inhibiting: true,
                        hotkey_overlay_title: None,
                    },
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_1,
                            ),
                            modifiers: Modifiers(
                                SHIFT | COMPOSITOR,
                            ),
                        },
                        action: FocusWorkspace(
                            Name(
                                "workspace-1",
                            ),
                        ),
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: false,
                        allow_inhibiting: true,
                        hotkey_overlay_title: None,
                    },
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_e,
                            ),
                            modifiers: Modifiers(
                                SHIFT | COMPOSITOR,
                            ),
                        },
                        action: Quit(
                            true,
                        ),
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: false,
                        allow_inhibiting: false,
                        hotkey_overlay_title: None,
                    },
                    Bind {
                        key: Key {
                            trigger: WheelScrollDown,
                            modifiers: Modifiers(
                                COMPOSITOR,
                            ),
                        },
                        action: FocusWorkspaceDown,
                        repeat: true,
                        cooldown: Some(
                            150ms,
                        ),
                        allow_when_locked: false,
                        allow_inhibiting: true,
                        hotkey_overlay_title: None,
                    },
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_s,
                            ),
                            modifiers: Modifiers(
                                ALT | SUPER,
                            ),
                        },
                        action: SpawnSh(
                            "pkill orca || exec orca",
                        ),
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: true,
                        allow_inhibiting: true,
                        hotkey_overlay_title: None,
                    },
                ],
            ),
            switch_events: SwitchBinds {
                lid_open: None,
                lid_close: None,
                tablet_mode_on: Some(
                    SwitchAction {
                        spawn: [
                            "bash",
                            "-c",
                            "gsettings set org.gnome.desktop.a11y.applications screen-keyboard-enabled true",
                        ],
                    },
                ),
                tablet_mode_off: Some(
                    SwitchAction {
                        spawn: [
                            "bash",
                            "-c",
                            "gsettings set org.gnome.desktop.a11y.applications screen-keyboard-enabled false",
                        ],
                    },
                ),
            },
            debug: Debug {
                preview_render: None,
                dbus_interfaces_in_non_session_instances: false,
                wait_for_frame_completion_before_queueing: false,
                enable_overlay_planes: false,
                disable_cursor_plane: false,
                disable_direct_scanout: false,
                restrict_primary_scanout_to_matching_format: false,
                force_disable_connectors_on_resume: false,
                render_drm_device: Some(
                    "/dev/dri/renderD129",
                ),
                ignored_drm_devices: [
                    "/dev/dri/renderD128",
                    "/dev/dri/renderD130",
                ],
                force_pipewire_invalid_modifier: false,
                emulate_zero_presentation_time: false,
                disable_resize_throttling: false,
                disable_transactions: false,
                keep_laptop_panel_on_when_lid_is_closed: false,
                disable_monitor_names: false,
                strict_new_window_focus_policy: false,
                honor_xdg_activation_with_invalid_serial: false,
                deactivate_unfocused_windows: false,
                skip_cursor_only_updates_during_vrr: false,
                disable_10bit_output: false,
            },
            workspaces: [
                Workspace {
                    name: WorkspaceName(
                        "workspace-1",
                    ),
                    open_on_output: Some(
                        "eDP-1",
                    ),
                    layout: None,
                },
                Workspace {
                    name: WorkspaceName(
                        "workspace-2",
                    ),
                    open_on_output: None,
                    layout: None,
                },
                Workspace {
                    name: WorkspaceName(
                        "workspace-3",
                    ),
                    open_on_output: None,
                    layout: None,
                },
            ],
            recent_windows: RecentWindows {
                on: false,
                debounce_ms: 750,
                open_delay_ms: 150,
                highlight: MruHighlight {
                    active_color: Color {
                        r: 0.0,
                        g: 1.0,
                        b: 0.0,
                        a: 1.0,
                    },
                    urgent_color: Color {
                        r: 1.0,
                        g: 0.6,
                        b: 0.6,
                        a: 1.0,
                    },
                    padding: 15.0,
                    corner_radius: 0.0,
                },
                previews: MruPreviews {
                    max_height: 960.0,
                    max_scale: 0.5,
                },
                binds: [
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_Tab,
                            ),
                            modifiers: Modifiers(
                                ALT,
                            ),
                        },
                        action: MruAdvance {
                            direction: Forward,
                            scope: None,
                            filter: Some(
                                All,
                            ),
                        },
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: false,
                        allow_inhibiting: true,
                        hotkey_overlay_title: None,
                    },
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_grave,
                            ),
                            modifiers: Modifiers(
                                ALT,
                            ),
                        },
                        action: MruAdvance {
                            direction: Forward,
                            scope: None,
                            filter: Some(
                                AppId,
                            ),
                        },
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: false,
                        allow_inhibiting: true,
                        hotkey_overlay_title: None,
                    },
                    Bind {
                        key: Key {
                            trigger: Keysym(
                                XK_Tab,
                            ),
                            modifiers: Modifiers(
                                SUPER,
                            ),
                        },
                        action: MruAdvance {
                            direction: Forward,
                            scope: Some(
                                Output,
                            ),
                            filter: Some(
                                All,
                            ),
                        },
                        repeat: true,
                        cooldown: None,
                        allow_when_locked: false,
                        allow_inhibiting: true,
                        hotkey_overlay_title: None,
                    },
                ],
            },
        }
        "#);
    }

    fn diff_lines(expected: &str, actual: &str) -> String {
        let mut output = String::new();
        let mut in_change = false;

        for change in diff::lines(expected, actual) {
            match change {
                diff::Result::Both(_, _) => {
                    in_change = false;
                }
                diff::Result::Left(line) => {
                    if !output.is_empty() && !in_change {
                        output.push('\n');
                    }
                    output.push('-');
                    output.push_str(line);
                    output.push('\n');
                    in_change = true;
                }
                diff::Result::Right(line) => {
                    if !output.is_empty() && !in_change {
                        output.push('\n');
                    }
                    output.push('+');
                    output.push_str(line);
                    output.push('\n');
                    in_change = true;
                }
            }
        }

        output
    }

    #[test]
    fn diff_empty_to_default() {
        // We try to write the config defaults in such a way that empty sections (and an empty
        // config) give the same outcome as the default config bundled with ymir. This test
        // verifies the actual differences between the two.
        let mut default_config = Config::load_default()
            .config
            .expect("embedded default config must parse");
        let empty_config = Config::parse_mem("").unwrap();

        // Some notable omissions: the default config has some window rules, and an empty config
        // will not have any binds. Clear them out so they don't spam the diff.
        default_config.window_rules.clear();
        default_config.binds.0.clear();

        assert_snapshot!(
            diff_lines(
                &format!("{empty_config:#?}"),
                &format!("{default_config:#?}")
            ),
            @r#"
        -            numlock: false,
        +            numlock: true,

        -            tap: false,
        +            tap: true,

        -            natural_scroll: false,
        +            natural_scroll: true,

        -    spawn_at_startup: [],
        +    spawn_at_startup: [
        +        SpawnAtStartup {
        +            command: [
        +                "waybar",
        +            ],
        +        },
        +    ],

        -                0.3333333333333333,
        +                0.33333,

        -                0.6666666666666666,
        +                0.66667,

        -        default_column_display: Normal,
        +        default_column_display: Dwindle,
        "#,
        );
    }
}
