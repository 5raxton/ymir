# Plan: Replace KDL config with Lua (data-table style)

## Summary

`ymir-config` is the only crate that parses KDL; the compositor only reads a `Config` struct (and a few runtime rule types). So we replace the **parsing layer** of `ymir-config` with an embedded Lua interpreter (`mlua`, vendored Lua 5.4), keep all the runtime types/`Config` intact, and delete all `knuffel`/KDL code. Config files become `config.lua`, hot reload still works via tracked `dofile`/`require`, and default/dwindle configs are shipped as `.lua`.

## Target Lua schema (checkpoints)

```lua
return {
  input = {
    keyboard = { repeat_delay = 600, repeat_rate = 25, track_layout = "window",
                 xkb = { layout = "us,ru", options = "grp:win_space_toggle" } },
    touchpad = { tap = true, dwt = true, click_method = "clickfinger",
                 scroll_factor = 0.9, disabled_on_external_mouse = true, ... },
    mod_key = "Mod5",
  },
  output = {
    { name = "eDP-1", scale = 2, transform = "flipped-90",
      position = { x = 10, y = 20 }, mode = "1920x1080@144",
      variable_refresh_rate = { on_demand = true }, hot_corners = { off = true }, ... },
  },
  layout = {
    focus_ring = { width = 5, active_color = "rgba(0,100,200,255)",
                   active_gradient = { from = "#0a1e2a", to = "rgba(0,99,180,1)", relative_to = "workspace-view" } },
    border = { width = 3, inactive_color = "rgba(255,200,100,0)" },  -- presence of key enables
    shadow = { offset = { x = 10, y = -20 } },
    preset_column_widths = { { proportion = 0.5 }, { fixed = 960 } },
    default_column_width = { proportion = 0.5 },
    gaps = 16, struts = { top = 0, left = 0, right = 0, bottom = 0 },
    center_focused_column = "on-overflow",
  },
  spawn_at_startup = { { command = { "alacritty", "-e", "fish" } } },
  prefer_no_csd = true,
  cursor = { xcursor_theme = "default", xcursor_size = 24, hide_when_typing = false },
  animations = { slowdown = 1, workspace_switch = { spring = { damping_ratio = 1, stiffness = 1000, epsilon = 0.0001 } } },
  binds = {
    { key = "Mod+Q", action = { name = "close-window" } },
    { key = "Mod+T", action = { name = "spawn", command = { "alacritty" } }, allow_when_locked = true },
    { key = "Mod+WheelScrollDown", action = { name = "focus-workspace-down" }, cooldown_ms = 150, repeat = true },
  },
  environment = { QT_QPA_PLATFORM = "wayland" },
  window_rules = {
    { match = { { app_id = ".*alacritty" } }, exclude = { { title = "~" } },
      open_on_output = "eDP-1", default_column_display = "tabbed", border = { width = 8.5 } },
  },
  layer_rules = { { match = { { namespace = "^notifications$" } }, block_out_from = "screencast" } },
  workspaces = { { name = "1", open_on_output = "eDP-1" }, { name = "2" } },
  recent_windows = { off = true, highlight = { padding = 15 }, previews = { max_height = 960 },
                     binds = { { key = "Alt+Tab", action = { name = "next-window" } } } },
}
```

Rules of thumb:
- KDL "flag node" (`tap` without value) → `tap = true`; `off`/`on` sections become `off = true` / `on = true`.
- Colors: accept csscolorparser strings **or** `{ r=, g=, b=, a= }` tables (map to existing `Color`).
- Enums (`transform`, `scroll-method`, `on-xdg-activate`, ...) are `FromStr`-parsed strings → reuse existing parsers.
- Action args (`arguments`/`properties`) map to named subfields in `{ name = ..., ... }`.

## Architecture changes

- **New engine** `ymir-config/src/lua.rs` (or `lua/mod.rs`): creates a fresh `Lua` per parse, installs a prelude global, runs the config chunk, returns the `mlua::Table`.
- **Prelude** provides: `include_config("path")` (load + deep-merge another config table), wrapped `dofile`/`require`/`loadfile` that record absolute loaded paths into a shared `Rc<RefCell<Vec<PathBuf>>>` (feeds the watcher), `~` expansion, and recursion detection (a stack, error on self-include). Optional includes via `pcall`. Broken includes still record their path first, so the watcher reloads once fixed (preserves current behavior).
- **Validation layer**: keep the compositor-facing runtime types (`Config`, `Layout`, `LayoutPart`, `BorderRule`, `ShadowRule`, `TabIndicatorRule`, `BackgroundEffectRule`, `PopupsRule`, `WindowRule`, `Output`, `Workspace`, `Bind`, `Action`, ...) but replace `#[derive(knuffel::Decode)]` with hand-written "apply from Lua table" functions per section (`apply_input`, `apply_layout`, `apply_binds`, ...). A shared helper module gives typed readers (`read_string`, `read_bool`, `read_range`, `read_enum`, `read_color`, `read_list`) and error accumulation — unknown keys/duplicate keys gather errors instead of failing fast, mirroring knuffel's style.
- **Error model**: replace `ConfigIncludeError`'s `knuffel::Error` with a new `ConfigError` (Lua runtime errors → miette with traceback text; validation errors → "section.key: msg"). Keep `ConfigParseResult { config, includes }` shape so `src/utils/watcher.rs` and `ConfigPath` stay near-unchanged.
- **Runtimes stay hold**: the `MergeWith` impls used outside parsing (rule resolution in `src/window/mod.rs`, `src/layer/*`, and `Layout::merge_with(LayoutPart)` in `src/layout/mod.rs:680`) are untouched; `LayoutPart` stays a runtime struct and gains a Lua-table constructor.

### What gets deleted
All `*Part` decode types and manual `Decode` impls (`ConfigPart`, `Bind`, `Action::decode`, `WorkspaceReference`, `WorkspaceName`, `FloatOrInt`/`Flag`/`Color` decode impls, `macros.rs` merge macros, `expect_only_children`/`parse_arg_node` — except any runtime-only `MergeWith` impls listed above), `knuffel`/`chomsky` deps, and the `[profile.release.package.ymir-config]` debug override.

## Phases

**Phase 1 — Engine + core plumbing**
- Add `mlua` (`features = ["lua54", "vendored"]`) to `ymir-config/Cargo.toml`; `cargo build` check.
- New `lua.rs`: runtime setup, prelude (`include_config`, tracked `dofile`/`require`, `~` expansion, recursion stack), error type `ConfigError` + `ConfigIncludeError` rework (`error.rs`).
- Rewrite `Config::parse`/`parse_mem`/`load` to run Lua; `ConfigPart`/include node handling deleted. Empty/returning-nil file ⇒ defaults.

**Phase 2 — Section appliers** (bulk, ~2–3k LOC, mechanical)
- Per section: `input`, `output`, `layout`, `appearance` (focus-ring/border/shadow/tab-indicator/insert-hint/struts/colors/gradients/blur), `animations`, `gestures`, `misc` (spawn, cursor, screenshot-path, clipboard, hotkey-overlay, overview, xwayland-satellite, environment), `debug`, `workspace`, `window_rule`, `layer_rule`, `binds` (Key parsing, Action table→enum incl. `Spawn`/`SpawnSh` arg forms, replace-by-key dedup semantics kept), `recent_windows`, `switch_events`.
- Preserve special semantics explicitly: `binds` replace-by-key; `layout` merges into `Layout` via `LayoutPart` for workspace/output `layout`; borders on/off quirk decided explicitly (`border = {...}` enables, `border.off = true` disables).

**Phase 3 — Harness updates**
- `ConfigPath::create` writes embedded `resources/default-config.lua`.
- `src/cli.rs`/`main.rs` help text, `scripts/install.sh`, `PKGBUILD`, `ymir.spec.rpkg`, `README.md` (dwindle link) → `.lua` references.
- `src/ui/config_error_notification.rs` wording stays (points to `ymir validate`), verify `validate` still reports Lua errors.

**Phase 4 — Configs, tests, docs-cleanup**
- Convert `resources/default-config.kdl` → `resources/default-config.lua` and `dwindle-config.kdl` → `resources/dwindle-config.lua` (settings preserved).
- Rework tests: delete all KDL-snippet tests in `ymir-config` (`lib.rs`, `appearance.rs`, etc.) and replace with Lua equivalents; convert the big `parse` snapshot test; rewrite `tests/wiki-parses.rs` (now `must-fail` semantics on the new default/dwindle `.lua` files instead of wiki [docs deferred]).
- Convert `src/utils/watcher.rs` tests (`.kdl` → `.lua`, contents → comment lines), `src/ui/hotkey_overlay.rs` tests (Lua `binds` strings), and `src/tests/window_opening.rs`/`floating.rs` setup configs.
- Verify hot reload: change main + included `.lua` files; run full `cargo test`.

**Phase 5 — Deferred (separate PRs):** docs/wiki Configuration:* rewrite to Lua.

## Risks / notes
- **Error line-numbers**: table values validated in Rust can't know their source line. Mitigation: Lua *runtime* errors get precise tracebacks; validation errors report `section.key` + message + (optionally) a source excerpt. Consider routing value validation through Lua helpers later for better spans.
- **Build time/size**: vendored Lua adds a little; one-time cost. Fresh state per parse avoids `Send` issues on the watcher thread.
- **Behavior parity**: bindcase handling (`XF86ScreenSaver`), dwell region defaults, `Percent`, preserve via existing `FromStr`/consts.
- **`SawMruBinds`** logic disappears — user table assignment replaces defaults deterministically.

## Verification
- `cargo test` for `ymir-config` + watcher + hotkey_overlay + tests suites.
- `target/release/ymir validate` against a sample `config.lua`; on-screen error notification path.
- Manual: launch with `default-config.lua`, edit + confirm hot reload incl. `include_config("colors.lua")` changes.

## Open decisions (asked before starting)
- ~~KDL legacy support~~ → decided: replace entirely.
- ~~Config API style~~ → decided: data-table return.
- ~~Includes model~~ → decided: Lua-native includes (`include_config`, tracked `dofile`/`require`).
- ~~Docs package~~ → decided: parser first, docs later.
- Remaining: color syntax, action shape (`{ name = ... }`?), `layout.border` enable semantics — confirm before Phase 2.