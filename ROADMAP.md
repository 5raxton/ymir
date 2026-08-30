# ymir — Hardening, Lua Configuration Engine & Depth-Queue Mode Roadmap

**Scope:** `ymir` (niri fork), workspace `main` @ `3016ec9`.
**Baseline:** `cargo check --workspace --all-features` passes at HEAD.
**Date:** 2026-08-30 · **Severity legend:** `CRIT` crash/corruption · `HIGH` visible breakage/panic on real input · `MED` edge-triggered bug · `LOW` cosmetic/debt.

This document has two halves:

1. **Part A — Audit Report.** The result of the deep-dive static audit of every high-risk subsystem (surface lifecycle, dwindle division, focus/workspace state machines, Xwayland/protocol sync, rendering/shaders, config parsing/hot-reload, IPC/input). Every finding is cited to `file:line`, either quoted or verified directly by the auditor.
2. **Part B — The Roadmap.** A four-phase, production-grade execution plan:

   - **Phase 1 — Bug Fixes & Refactoring Hardening.** Remediate every crash vector, race condition and rendering fault from Part A, then pay down the structural debt (monoliths, duplicated geometry code, dead config paths).
   - **Phase 2 — Lua Configuration Engine Overhaul.** Strip KDL/knuffel entirely; ship an `mlua`-backed, event-driven, hot-swappable config runtime at `~/.config/ymir/init.lua` with an imperative `ymir` API (`ymir.bind`, `ymir.action`, Lua window rules) plus a zero-downtime file-watcher.
   - **Phase 3 — Depth-Queue Mode.** The third native display mode: an interactive, depth-sorted card stack with custom shaders and spring-animated focus shuffling, fully wired into layout, rendering, config, keybindings and `ymir msg`.
   - **Phase 4 — Verification, Testing & Polish.** The full validation matrix, GPU budget, docs/packaging updates and the release checklist.

---

## Part A — Audit Report

### A.1 Methodology

Four parallel static audits were run over `src/`, `ymir-config/`, `ymir-ipc/`, `resources/`, `scripts/`, `docs/` and packaging files:

| # | Area | Focus |
|---|------|-------|
| 1 | Backend & window management | Surface lifecycle/destruction races, dwindle division & min-size clamping, focus/workspace hotplug state machines, Xwayland-satellite + protocol sync |
| 2 | Rendering & UI pipeline | Oklab/Oklch gradient borders, blur & spring animation, NVIDIA/mixed-DPI scaling, GLSL shader audit |
| 3 | Config system | ymir-config crate inventory, runtime types, KDL/knuffel usage, watcher/hot-reload, Lua-migration blast radius |
| 4 | IPC, input, nomenclature | IPC architecture, input grabs/races, niri remnants, dead code & structural debt, action surface |

All high-severity claims were re-verified by a second, direct read of the cited code. Every finding in the table below was then re-checked against the tree at HEAD (`3016ec9`) on 2026-08-30 as the report was finalized; rows whose original characterization needed a factual correction carry an inline `verified w/ correction` note (**B2, B7, B8, R6, R7, I3, I4, S1, S2, plus the matches!-branch count in A.6**), and every other row was verified as written. Corrections are limited to line-characterization issues (a wider-than-claimed unwrap set, delegation vs. duplication, subsisting-but-stale test coverage, an unreachable-now-but-defensible unwrap) — none change the Phase 1 remediation plan. `github.com/smithay/smithay` (rev `4cf0b62`) is pinned; no open-source CVE scan was performed, but the renderer/security-relevant smithay call sites were reviewed for misuse patterns.

### A.2 Findings summary

| ID | Severity | Subsystem | Finding | Location |
|----|----------|-----------|---------|----------|
| B1 | `MED` | Layout/grab | Interactive **move** updates panic when the dragged window is destroyed mid-grab (no `alive()` re-check in `Move`/`InteractiveMove` states). Resize path is safe; move is not. | `layout/mod.rs:4050-4055`, `input/move_grab.rs:224`, `input/touch_overview_grab.rs:155` |
| B2 | `MED` | Workspace | `set_maximized` uses three `find(...).unwrap()` on `scrolling.tiles()` (`:1420`, `:1435`, `:1445` — the report originally counted two); stale/closed window IDs (IPC `maximize-window`, close-during-transition) panic. | `layout/workspace.rs:1420-1424,1435-1439,1444-1449` |
| B3 | `MED` | Workspace | `add_tile(NextTo)` panics via `active_window().unwrap()` and `find(...).unwrap()` when focus is `None` or the target is floating/gone. | `layout/workspace.rs:691,704` |
| B4 | `MED` | Dwindle | Tree↔tiles desync panics: raw `leaf_paths()[tile_idx]` OOB and `dwindle_tree.expel(&path).unwrap()`; `set_active(&paths[...])` underflow guard (`active_tile_idx = tile_idx - 1`) is invariant-dependent. | `layout/scrolling.rs:1230-1231,1263,4751` |
| B5 | `MED` | Resize grab | `interactive_resize_update` unwraps column/tile lookups mid-grab; a column move or tab-cycle during an active resize grab panics on next pointer motion. | `layout/scrolling.rs:3921-3938` |
| B6 | `MED` | Layer-shell | `assert!(is_new)` and `map_layer().unwrap()` on every new layer surface; fires if a `wl_surface` maps twice or an output disconnects mid-commit. | `handlers/layer_shell.rs:40,44` |
| B7 | `LOW` | Workspace | `update_output_size` does `output.as_ref().unwrap()`; `None` output is a legal whole-system state (all displays off). `verified w/ correction`: no current caller routes a `NoOutputs` workspace here (`monitor.rs:1239` only iterates monitor workspaces), so it is unreachable today — treat as a defensible unwrap, not a live panic. | `layout/workspace.rs:549` |
| B8 | `LOW` | Hotplug | The `NoOutputs ⇄ Outputs` transition matrix (`MonitorSet` state ops) is the highest-churn state machine. `verified w/ correction`: the original "no dynamic hotplug test coverage" claim was stale — `AddOutput`/`RemoveOutput` ops + the proptest matrix (`layout/tests.rs:761,831,1664-1670`) and `src/tests/remove_output.rs` do cover it, though the end-to-end test never enters `NoOutputs`; the sticky-empty-workspace invariant still rests on review plus those op tests. | `layout/mod.rs:877-1470` |
| R1 | `HIGH` | Shader | **Oklab gradient NaN.** The oklab branch does not clamp `oklab_to_linear(...)` output (oklch branch does). Interpolation between in-gamut sRGB colors can exit the sRGB gamut in Oklab, yielding negative LMS → `pow(neg, 3)` / `pow(neg, 1/2.2)` → NaN, which poisons the whole border tile (NaN propagates through `clamp`/`mix`). | `render_helpers/shaders/border.frag:144` vs `:190` |
| R2 | `HIGH` | GLES | `GL_CLAMP_TO_BORDER` set unconditionally on every `ShaderRenderElement` texture; smithay's default context is GLES 2.0 where the enum is invalid (Mesa: silently ignored; NVIDIA EGL 3.x: valid, but border color never set → black edges on the resize/close crossfade tex-coords). No capability check. | `render_helpers/shader_element.rs:392-403` |
| R3 | `MED` | Animation | Spring `duration()` returns the *envelope*, not the *value*, crossing for underdamped/critical springs → residual up to ~4 px at the end of long workspace scrolls; `Animation::value_at` then snaps to `to`. End-of-scroll "jump" artifact. | `animation/spring.rs:66-72`, `animation/mod.rs:255-257` |
| R4 | `MED` | Shader | `pow(c, 2.2)` fake sRGB EOTF (no linear foot) in both directions; Oklab/Oklch endpoints drift from CSS/SVG reference values by 2-3% in shadows. | `render_helpers/shaders/border.frag:62-68` |
| R5 | `MED` | Shader | Oklch path clamps each linear-RGB channel independently → hue-distorted "gray band" segments when crossing the gamut boundary (companion to R1; recommend one gamut-mapping fix for both). | `render_helpers/shaders/border.frag:190` |
| R6 | `MED` | Blur | Edge halo: `blur_down.frag`/`blur_up.frag` sample up to ~1 blur-radius beyond the captured region with `GL_CLAMP_TO_EDGE` symmetric extension → bright window edges bleed inward over dark backdrops. Corner-mask `+1px` HACK in `background_effect.rs` only compensates, does not fix. `verified w/ correction`: the HACK lives in `render_helpers/background_effect.rs:66-73`, not `handlers/`. | `render_helpers/blur.rs:257-266,320-329`, `render_helpers/shaders/blur_*.frag`, `render_helpers/background_effect.rs:66-73` |
| R7 | `LOW` | Blur | Live resize/move reallocates the blur mip chain and reruns `passes+1` full-screen passes on change; bounded but expensive on NVIDIA. `verified w/ correction`: the original "reallocates every changed frame" was overstated — `blur.rs:121-138` only reallocs when the size changed or the texture is non-unique, so it is inherent to *resize* (size changes each frame), not to move. Aspect-aware texture reuse suggested. | `render_helpers/blur.rs:121-138,202-338` |
| R8 | `LOW` | Scale | All element matrices computed in `f32`; sub-texel drift when a single window spans two monitors with different fractional scales. | `render_helpers/resize.rs:56-71` |
| R9 | `LOW` | Shader | Unused `uniform vec2 ymir_size` declared and uploaded in `border.frag`. | `render_helpers/shaders/border.frag:10`, `shader_element.rs:424` |
| I1 | `MED` | Input | **Stale keybind repeat across config reload.** `start_key_repeat` and `bind_cooldown_timers` capture *cloned* binds; `reload_config` swaps config and rebuilds keymap sets but never cancels the repeat timer, so a key held across a reload keeps firing the pre-reload action until release. | `input/mod.rs:604,632-634,674-693`; `ymir.rs:1553-1565` |
| I2 | `LOW` | Input | `ScrollTracker` residual accumulator is not reset on config reload; leftover `acc`/`last` bleeds into post-reload scrolling. | `input/mod.rs:3551-3552`, `input/scroll_tracker.rs` |
| I3 | `LOW` | IPC | `send_blocking` inside main-loop idle callbacks — `Request::Layers` (`:325`), `Request::Action` (`:392`), and *also* `Request::FocusedOutput` (`:441`, missed by the original report), each bounded(1) — low risk, but the scheduler can stall a frame if the receiver is not polled yet. | `ipc/server.rs:325,392,441` |
| I4 | `LOW` | Seat | `reload_config` calls `self.ymir.seat.get_keyboard().unwrap()` — panic if a seat has no keyboard at reload time. `verified w/ correction`: only reached when repeat settings changed (`ymir.rs:1519-1521`), so the panic is gated on a repeat-rate/delay diff, not every reload. | `ymir.rs:1522` |
| C1 | `LOW` | Config | `Config::load_default()` contains an unconditional `assert!` on the embedded default (boot/exit-confirm path only, but it is an unconditional panic). | `ymir-config/src/lib.rs:470` |
| S1 | `LOW` | Debt | Monoliths confirmed: `layout/scrolling.rs` (6586), `ymir.rs` (6576), `input/mod.rs` (5695), `layout/mod.rs` (5184). `verified w/ correction`: the original "display-mode logic duplicated across `Column`/`ScrollingSpace` (`scrolling.rs:2454` vs `5886`)" is *delegation* — `:2454` is a wrapper forwarding to `Column::set_column_display` at `:5886`, with only a duplicated `display_mode == display` early-return guard. Tile sizing/animation primitives *are* reimplemented in `tile.rs` and `scrolling.rs`. | §A.6 |
| S2 | `LOW` | Debt | `verified w/ correction`: the "6 residual niri references" count was stale — only 2 remain (`README.md:24` fork attribution, `:172` upstream link), plus upstream author metadata (`Cargo.toml:11`); the wiki COPR links are already cleaned. Not a naming leak into client-facing surfaces. | §A.6 |

**Notable non-findings (verified safe):** dwindle divider math is pixel-exact (`first_w = floor`, other child absorbs remainder; covered by `leaf_rects_share_divider_on_whole_pixels` tests); the min-size clamp is correctly *only* reachable interactively via `adjust_ratio_for_edge`; the layout/MRU destruction ordering in `toplevel_destroyed` removes the MRU entry before `layout.remove_window`; `XwaylandSatellite` setup re-requests are marshalled to the main loop (no cross-thread mutation); `ext_workspace.rs` group data is always set at creation; `Transaction` deadline logic is single-threaded/non-blocking; blur textures are only recaptured when the backdrop is damaged; blur shaders are `#version 100`-consistent with the central `#version 100` header; `handle_focus_follows_mouse` has an explicit `focus != current` anti-feedback-loop guard.

### A.3 Backend & window management — detail

- **B1 (move-grab dead-window panic).** `MoveGrab::on_frame` checks `self.window.alive()` **only** in `Recognizing` state (`input/move_grab.rs:184-188`); once in `Move` it calls `interactive_move_update` unconditionally. `TouchOverviewGrab` has the same gap (`input/touch_overview_grab.rs:155-167`). The layout-side `interactive_move_update` `Starting` branch crashes on survivors of a mid-grab close:

  ```rust
  let (is_floating, tile, workspace_config) = self
      .workspaces_mut()
      .find(|ws| ws.has_window(&window_id))
      .map(|ws| {
          ...
          ws.tiles_mut().find(|tile| ...window_id).unwrap(),   // mod.rs:4051
      })
      .unwrap();                                                // mod.rs:4055
  ```
  The *end* paths are safe (`interactive_move_end` and `interactive_resize_end` both check `has_window`); the *update* path is the vulnerability. Fix: add `alive()` re-checks in `Move`/`InteractiveMove` frames (`input/move_grab.rs:224`, `touch_overview_grab.rs:155`) and turn the layout-side unwraps into `let Some(...) = ... else { interactive_move = None; return true; }` recovery. Same hardening pattern for B2/B3/B5.

- **B4 (dwindle desync).** `remove_tile_from_dwindle` (`scrolling.rs:1230`) indexes `leaf_paths()[tile_idx]` and `expel(...).unwrap()`; `activate_idx` (`scrolling.rs:4751`) repeats the raw index. The guarding invariant is concentrated in `reorder_tiles_by_dfs` (`scrolling.rs:6180-6187`, `debug_assert_eq!(order.len(), tiles.len())`). Any future mutation of `tiles[]` without reindexing reopens the panic. Add `leaf_paths().len() == tiles.len()` invariant checks and replace raw index + `unwrap` with message-bearing `get()...expect(...)`/conditional guards.

### A.4 Rendering, shaders & UI — detail

- **R1 (NaN in Oklab) is the top rendering fix.** The oklch branch at `border.frag:190` clamps; the oklab branch at `border.frag:144` does not. `oklab_to_linear` computes `pow(lms, vec3(3.0))`; if interpolation leaves the sRGB gamut, LMS can become negative and `pow` is undefined (NaN) on every GPU; one NaN fragment NaN-poisoning the whole tile via the final `premul_rect`. Fix R1, R4 and R5 together: (a) piecewise sRGB EOTF, (b) a single gamut-reduction step for the Oklab family (hue-preserving LCh gamut map) replacing the raw per-channel clip, (c) guard `lms = max(lms, 0.0)` as a belt-and-suspenders.

- **R2 (CLAMP_TO_BORDER).** `shader_element.rs:392-403` applies `TEXTURE_WRAP_S/T = CLAMP_TO_BORDER` to every custom-shader texture without checking a GLES2 vs GLES3 context or the `GL_EXT_texture_border_clamp` extension. smithay's default EGL path requests a GLES 2.0 context; on NVIDIA the context is GLES3 (valid enum) but `GL_TEXTURE_BORDER_COLOR` is never set → transparent-black borders sampled outside `[0,1]`, i.e. the dark sweep on resize/open/close animations. Fix: capability/extension probe (`Capability`-style, like the existing instancing check at `shader_element.rs:312`), fall back to `CLAMP_TO_EDGE`, and set `TEXTURE_BORDER_COLOR` when supported.

- **R3 (spring end-snap).** `duration()` (spring.rs:66-72) returns the first time the *envelope* is below epsilon, but the underdamped value residual is `eps·ω₀/ω₁` (≈1.25×eps at damping 0.6) and the critical residual is `eps·(1+β·t)`, i.e. up to ~4 px at 4K scroll distances. Two fixes (either/both): compute the true value-crossing time with the Newton refinement already used for the overdamped branch, and/or make `Animation::value_at` return `to` beyond the clamped duration (the `clamped_value()` path) so call sites using `value()` stop seeing the tail jump.

### A.5 Config system & Lua-migration readiness — detail

- **Blast radius:** KDL parsing lives **only** in `ymir-config`. `knuffel 3.2.0` is the sole parsing dep (`ymir-config/Cargo.toml:13`); **`chomsky` is NOT a real dependency** (`FUTURELUAPLAN.md:69` overstates this — only knuffel needs removal; the `[profile.release.package.ymir-config]` drop of debuginfo at root `Cargo.toml:157-159` becomes unnecessary).
- **`mlua` is not present** anywhere (no Cargo.toml, no Cargo.lock entry, no `resource/*.lua`).
- **Runtime types that survive parsing** and their `MergeWith` runtime call sites: `Layout←LayoutPart` (`layout.rs:59`, consumed at `src/layout/mod.rs:696-701`, `src/layout/tests.rs:1622,4852`), `Border←BorderRule`/`FocusRing←BorderRule` (`appearance.rs:315-339`, consumed at `layout/tile.rs:190-192`, `layout/workspace.rs:937`), `Shadow←ShadowRule` (`appearance.rs:369-382`, `layer/mod.rs:79`, `layer/mapped.rs:80,100`), `TabIndicator` (`appearance.rs:501-527`), `InsertHint` (`appearance.rs:596-605`), `Blur←BlurPart` (`appearance.rs:1046`), `BackgroundEffect←BackgroundEffectRule` (`appearance.rs:1092`, `window/mod.rs:314-317`, `layer/mod.rs:81-85`), `ResolvedPopupsRules` (`window_rule.rs:110`), `SwitchBinds`, `Input←InputPart` (`input.rs:58`), `Animations←AnimationsPart`, `Debug←DebugPart`, `Gestures←GesturesPart`, `RecentWindows←RecentWindowsPart`, plus the delegate decoders `Color::from_str`, `Key::from_str`, `ModKey`, `Percent`, `FloatOrInt`, `Flag`, `CornerRadius`.
- **Decode-tagged runtime hold types** that the plan under-weights: `Output` (`output.rs:50`), `Workspace` (`workspace.rs:5`), `WindowRule`/`LayerRule`/`Match`, `SwitchBinds`/`SwitchAction`, all `Input` sub-types (`input.rs:187,227,251,273,359,375`; note the *top-level* `Input`/`Keyboard` themselves do not derive — their `*Part` twins do), `Environment`, `HotCorners`, all `Mru*Part` — these carry `#[derive(knuffel::Decode)]` directly and need Lua-table constructors, not just deletion.
- **Hot-reload today:** a polling watcher thread (`src/utils/watcher.rs`, 500 ms interval) parses config off-loop and ships `Result<Config,()>` through `calloop::channel::sync_channel(1)`; on parse failure it keeps the old config and shows the on-screen notification (`src/ui/config_error_notification.rs`, `error_text` at `:250-258` — the function formats `ymir validate`, which is already correct for the post-migration command name). Includes are re-fed to the watcher after every reload (`set_includes`); there is a documented include-set race window (`watcher.rs:100-104`).
- **Quirks to preserve exactly:** binds replace-by-key; the `layout { border {} }` enable / `off` disable quirk (recursion==0); numeric workspace tags (`workspace_default_index`, which lives at `src/layout/mod.rs:388`, not in `ymir-config`); `allow-when-locked` only on spawn; `SawMruBinds`; `XF86ScreenSaver` case handling.
- **Config-path surface to migrate:** `src/main.rs:345,350`; `src/cli.rs:16,49`; `ymir-config/src/lib.rs:109,111,500,535`; `resources/default-config.kdl` & `dwindle-config.kdl`; `ymir.spec.rpkg:157`; `scripts/install.sh:10,22,241-245`; `README.md:105,116,136`; test suites in `src/utils/watcher.rs:379-753`, `src/ui/hotkey_overlay.rs:620-709`, `src/tests/window_opening.rs:309,604,775`, `src/tests/floating.rs:911,1212,1256`, **`src/layout/tests.rs:2364,2447`** (missing from the current plan's Phase 4 list) and `ymir-config`'s own KDL-snippet tests incl. `tests/wiki-parses.rs`.
- **`FUTURELUAPLAN.md` discrepancies vs. the tree:** `source/layout/mod.rs:680` in the plan is the compositor call (`mod.rs:696-701`), not the definition (`ymir-config/src/layout.rs:59`); several current schema sections are absent from the plan's sample table (`insert-hint`, `preset-window-heights`, `config-notification`, `blur`, `overview`, `xwayland-satellite`, `switch-events`, `debug`, `gestures`); `src/layout/tests.rs` is unlisted in the harness migration.

### A.6 IPC, input, nomenclature & structural debt — detail

- **IPC** is a Unix-socket JSON-line protocol (`$XDG_RUNTIME_DIR/ymir.$WAYLAND.$PID.sock`), cooperative on the single main loop with a `calloop::Scheduler`. Event fan-out is correctly **bounded** (`async_channel::bounded(64)`, slow clients are disconnected, `ipc/server.rs:42,119-135`). `Request::EventStream` swallows the connection (two sockets required for requests+events — documented `ymir-ipc/src/lib.rs:7-9`). Malformed input returns `Reply::Err` (no panic). The `insert_idle`+`send_blocking` pattern (I3) is the only stall exposure — **three** rendezvous sites: `Request::Layers` (`:325`), `Request::Action` (`:392`), and `Request::FocusedOutput` (`:441`), plus the slow-client `disconnect.send_blocking` at `:134`.
- **Column display-mode surface** (the integration point for depth-queue): `ColumnDisplay` enum `ymir-ipc/src/lib.rs:1019-1026` (`Normal | Tabbed | Dwindle`); the action enum `ymir-ipc` (`SwitchColumnDisplay`, `SetColumnDisplay`, `ToggleColumnTabbedDisplay`), config-converted at `ymir-config/src/binds.rs:563-565`, dispatched at `input/mod.rs:1646-1663`, implemented `layout/mod.rs:2355` → `scrolling.rs:2454,2464,5886`; cycle matrices `scrolling.rs:2431-2433` and `2446-2448`; per-column `display_mode` field `scrolling.rs:211`. A new mode must be threaded through all of these plus **21** display-mode branch sites (`scrolling.rs:508,534,882,983,1145,1676,2482,2831,3185,3216,3324,4331,4781,4935,5046,5682,5733,5765,6086,6144,6257`) — the original "~17 `matches!(...)`" phrasing was misleading: only two use `matches!` (`:2482`, `:6144`), the other 19 are `==` comparisons.
- **niri remnants are confined to 2 lines + metadata**, all documentation/attribution: `README.md:24` (fork attribution), `README.md:172` (upstream URL), `Cargo.toml:11` (upstream author email). The previously-cited wiki COPR links (`docs/wiki/Development:-Animation-Timing.md:23`, `Development:-Releasing-ymir.md:66`, `Getting-Started.md:48`) are already cleaned. Zero leaks in `resources/*`, `scripts/*`, `websrc/*`, `PKGBUILD`, `.spec`, IPC, desktop/service files or env vars — confirmed by a case-insensitive `niri` sweep over everything except `target/`, `.git/` and this roadmap.
- **Dead code:** none flagged `#[allow(dead_code)]` except auto-generated `protocols/raw.rs`. Largest files: `scrolling.rs` (6586), `ymir.rs` (6576), `input/mod.rs` (5695), `layout/mod.rs` (5184), `layout/tests.rs` (4906).

---

## Part B — The Roadmap

### Phase 1 — Bug Fixes & Refactoring Hardening

**Goal:** eliminate every crash/panic vector, animation artifact and GPU fault from Part A, and cut the structural debt that would otherwise slow Phases 2-3. No behavioral changes beyond bug fixes.

#### 1.1 Crash-safety hardening (grab races + stale-id panics)

1. **Move-grab stale window (B1).** Add `self.window.alive()` re-checks in `MoveGrab::on_frame` (`input/move_grab.rs:224`) and `TouchOverviewGrab::on_frame` in `InteractiveMove` (`input/touch_overview_grab.rs:155`); on death, end the grab cleanly (`interactive_move_end` already tolerates unknown ids). Convert the `find(...).unwrap()`s in `layout/mod.rs:4050-4055` to `let Some(...) = ... else { self.interactive_move = None; return true; }`.
2. **Maximize/unmaximize stale-id (B2).** Both `find(...).unwrap()` sites (`layout/workspace.rs:1420-1424`, `1435-1439`) become `if let Some(tile) = ... { ... }` / early-return; the tile is gone or floating → treat as no-op (matches the existing floating-window early return above them).
3. **`add_tile(NextTo)` (B3).** Replace `active_window().unwrap()` (`workspace.rs:691`) and the `find(...).unwrap()` (`workspace.rs:704`) with match/`if let`; invalid `next_to` (floating or absent) degrades to `Bellow` insertion mirroring the `InFocus` behavior.
4. **Dwindle desync invariants (B4).** Add `debug_assert_eq!(column.dwindle_tree.leaf_paths().len(), column.tiles.len())` after every tree mutation (`dwindle_tree.open_new/preselect/toggle_split/expel/...` call sites); replace the raw `leaf_paths()[tile_idx]` + `expel(...).unwrap()` (`scrolling.rs:1230-1231,1263`) and `activate_idx` (`scrolling.rs:4751`) with guarded lookups carrying a descriptive panic message. Add a unit test that fuzzes tree mutations + per-mutation `reorder_tiles_by_dfs` to prove the invariant (extend the existing proptest harness in `src/layout/tests.rs`).
5. **Resize-grab mid-grab recovery (B5).** Guard `interactive_resize_update`'s column/tile lookups (`scrolling.rs:3921-3938`) with `let else`; on failure clear `self.interactive_resize` and return `false` (the caller already tolerates `false` from `HasWindow` misses).
6. **Layer-shell double-map (B6).** Replace the `assert!(is_new)` (`handlers/layer_shell.rs:40`) with an idempotent `insert` + `warn!` on the duplicate-commit race; downgrade `map_layer(...).unwrap()` to a handled-`if let Err` path using the layer's error type, treating a gone output as "send close".

#### 1.2 Rendering & shader remediation

1. **Oklab NaN + gamut handling (R1, R4, R5).** In `render_helpers/shaders/border.frag`:
   - Replace both `pow(c, 2.2)`/`pow(c, 1/2.2)` helpers (`:62-68`) with the piecewise sRGB EOTF and its inverse.
   - Add `lms = max(lms, vec3(0.0))` inside `oklab_to_linear` (`:109-121`) as a guard.
   - Apply the same hue-preserving gamut reduction to **both** the oklab (`:144`) and oklch (`:190`) branches — desaturate-until-in-gamut in LCh (perceptual), not a raw per-channel clip. Add a CPU-side unit test (extend `render_helpers` tests) that drives known out-of-gamut chord endpoints and asserts finite, in-gamut output.
2. **CLAMP_TO_BORDER capability (R2).** In `shader_element.rs:392-403`, probe for border-clamp support (extension/version, patterned on the existing `Capability::Instancing` check); set `GL_TEXTURE_BORDER_COLOR` explicitly when used; otherwise fall back to `CLAMP_TO_EDGE`. Verify on llvmpipe *and* NVIDIA.
3. **Spring end-of-animation snap (R3).** In `animation/spring.rs`, compute `duration()` for the underdamped/critical branches via the same Newton value-crossing refinement used for the overdamped branch; ensure `Animation::value_at` clamps cleanly to `to` past the computed duration (`animation/mod.rs:255-257`). Add a `duration()` unit test asserting `|value(duration) - to| <= epsilon` for ratio `{0.3, 0.6, 0.8, 1.0}`.
4. **Blur edge halo (R6).** Extend the captured region by one blur radius (or clamp sample coordinates to the valid data) and mask the halo against the corner clip; retire the `+1px` radius HACK at `render_helpers/background_effect.rs:68` once the halo is fixed. Add a visual headroom constant so the capture rect already includes the extra ring during `prepare_textures`.
5. **Blur mip reuse during live resize (R7).** Reuse the mip chain when the effect aspect-ratio still maps into the same ½ⁿ scale chain; only re-clear (`blur.rs:121-138`) when the chain actually changes.
6. **f32 matrix drift (R8).** Keep rendering matrices in `f64` until the final `to_physical_precise_round` where possible (`resize.rs:56-71`); add a mixed-DPI visual-test scenario.
7. **Dead uniform (R9).** Remove `uniform vec2 ymir_size` from `border.frag` and its upload in `shader_element.rs:424`.

#### 1.3 Input state hygiene

1. **Stale keybind repeat (I1).** Track the bind currently being repeated; on `reload_config` (and on any change to the `binds` keymap) cancel `bind_repeat_timer` and clear `bind_cooldown_timers` for keys whose binding changed (`input/mod.rs` + `ymir.rs:1553-1565`). This is also a **pre-requisite for Phase 2's hot-swap** (a swapped-in global keymap must not keep firing stale actions).
2. **Scroll tracker reset (I2).** Reset `ScrollTracker`/gesture accumulators when the config is reloaded (`input/mod.rs:3551-3552`).
3. **Seat-keyboard unwrap (I4).** Replace `seat.get_keyboard().unwrap()` with an `if let` in the reload path (`ymir.rs:1522`).

#### 1.4 Structural cleanup

1. **De-monolith before Phase 3.** Split, without behavior change:
   - `layout/scrolling.rs` → `scrolling/{mod,column,tile,display,dwindle,animation}.rs` (fold the depth-queue work into `display.rs`).
   - `input/mod.rs` → `input/{mod,keyboard,pointer,touch,gestures,grabs}.rs`.
   - `layout/mod.rs` → extract `render.rs` (the element/render pipeline ~1k LOC) and `grab.rs` (interactive move/resize state).
   - `ymir.rs` → extract `state/config.rs`, `state/reload.rs`, `state/outputs.rs`; keep the seat/handlers in `State`.
2. **Deduplicate tile geometry/animation.** Move the width/height conversions (`tile.rs:962-992` vs `scrolling.rs` inline math) and move-animation helpers (`tile.rs:589-665` vs `scrolling.rs:587-810`) into one shared `geometry.rs`; make `scrolling.rs`'s `TileData`/`ColumnData` bookkeeping single-sourced on `Tile`.
3. **Single display-mode router.** Introduce `ColumnDisplay`-aware `ColumnLayoutLogic` trait or a match-once dispatcher so `set_column_display`, the cycle matrices and the `matches!(display_mode, ...)` fans all live in one module. This is the seam Phase 3 plugs `Depth` into.
4. **niri remnants (A.6).** Re-point the COPR links in `docs/wiki/*` and replace upstream author metadata (`Cargo.toml:11`) with fork maintainer contact.
5. **Config hardening laterals (C1).** Make `Config::load_default`'s `assert!` (`ymir-config/src/lib.rs:470`) a parse-result error so a broken embedded default is diagnosable rather than a boot panic.

**Exit criteria:** `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check` green; no panic-capable unwrap remains on: pointer-motion, key-binding, surface-commit or output-hotplug paths (re-grep + CI gate, see Phase 4); on-screen visual check of gradient borders, blur corners, and end-of-scroll under 1.5× fractional scale.

#### Phase 1 — status

Landed in the initial Part B pass (on top of `2df9499`):

- **1.1** B1–B6 complete (`move_grab.rs`/`touch_overview_grab.rs`/`interactive_resize_update` guarded lookups; `set_maximized` and `add_tile(NextTo)` stales; dwindle invariant asserts + fuzz proptest; layer-shell idempotent map + handled `map_layer` error).
- **1.2** R1/R4/R5 (`border.frag` piecewise EOTF, negative-LMS guard, LCh-invariant gamut reduction for both oklab/oklch arms + CPU-side mirror tests in `render_helpers/mod.rs`), R2 (GLES/extension border-clamp probe with `CLAMP_TO_BORDER` ↔ `CLAMP_TO_EDGE` fallback), R3 (spring `duration()` Newton refinement for all damping regimes + ratio-sweep unit test), R6 (coverage-weighted blur taps retire the +1px HACK), R7 (mip-chain reuse in `prepare_textures`), R8 (f64 matrices up to the final `to_physical_precise_round`), R9 (`uniform vec2 ymir_size` removed).
- **1.3** I1 (bind repeat/cooldown timers cancelled on `binds_changed`), I2 (scroll-tracker reset on every config reload), I4 (`get_keyboard().unwrap()` → `if let`) — all in the `reload_config` path.
- **C1** `Config::load_default` now returns a miette `ConfigParseResult` (no boot `assert!`).
- **1.4** niri metadata: `Cargo.toml` author re-pointed to the fork maintainer; wiki COPR links were already cleaned (A.6). Tile width/height conversions are single-sourced on `Tile` already (verified: `scrolling.rs` calls the `tile.*_for_*` methods, no inline border math).
- With the first clippy gate: `dwindle.rs` dead/needless patterns and `LeafPath::child` removed, `clamp_ratio` → `.clamp()`, the dwindle invariant checker is exercised by the proptest.

Verification: `cargo test --workspace` (273/273) and `cargo clippy --workspace --all-targets -- -D warnings` green. `cargo fmt --check` caveat: `rustfmt.toml` uses nightly-only options (`imports_granularity`, `group_imports`, `wrap_comments`, `comment_width`) and this tree only has stable rustfmt, so whole-tree `fmt --check` reports pre-existing nightly-style files; edited files were formatted with stable rustfmt.

Deferred to the follow-up structural passes (behind Phase basic hardening, non-blocking for the Phase 1 exit above):
- 1.4 item 1 (de-monolith file splits: `scrolling.rs`, `input/mod.rs`, `layout/mod.rs`, `ymir.rs`) — planned before Phase 3.
- 1.4 item 3 (single display-mode router / `ColumnDisplay`-aware dispatcher) — the seam Phase 3 plugs `Depth` into.
- 1.4 item 2 remainder: `scrolling.rs` `TileData`/`ColumnData` bookkeeping single-sourced on `Tile`, and tile/animation helper consolidation into a shared `geometry.rs` (width/height conversions already centralized).

---

### Phase 2 — Lua Configuration Engine Overhaul

**Goal:** delete every line of KDL/knuffel and replace it with an `mlua` (vendored Lua 5.4) runtime that is hot-swappable, event-driven and safe, loading `~/.config/ymir/init.lua`. This subsumes `FUTURELUAPLAN.md` (its data-table `return {...}` schema becomes one of two supported styles) and extends it with the imperative `ymir` API and a "rule/event" runtime that can react to window events, not just static tables.

#### 2.0 Architecture

```
┌───────────────────────── main loop (single-threaded) ─────────────────────────┐
│  State (ymir.rs) ── RefCell<Config>  ── keymap/rule tables (live, diff-driven) │
│   ▲                         ▲                          ▲                        │
│   │ ConfigParseResult      │ msg(reload)             │ Msg(lua commands)        │
│   │  {config, includes}    │ calloop::channel        │ (bounded channel)        │
├── ┼─────────────────────────┼──────────────────────────┼────────────────────────┤
│  ┌┴────────────────────────────────┴───────────────────────────┴────────────┐ │
│  │                    Lua runtime thread (owns the mlua VM)                 │ │
│  │  · prelude: include_config / tracked dofile,require / ~ · fresh sandbox  │ │
│  │  · compile init.lua → ConfigProgram { settings, binds, rules, hooks }     │ │
│  │  · notify::RecommendedWatcher → recompile on save (zero-downtime)         │ │
│  │  · event runner: window.new / destroyed / focused, workspace.set →        │ │
│  │    evaluates user hooks, emits Vec<LuaCommand> back to the main loop      │ │
│  └───────────────────────────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────────────────────┘
```

Design decisions (resolving `FUTURELUAPLAN.md` open points + Part A findings):

- **A dedicated config/runtime thread owns the VM.** `mlua::Lua` stays off the main loop, so rule evaluation (potentially user-authored, possibly slow) can never stall rendering/input. The main loop and the VM only exchange `Config`/`ConfigProgram` snapshots and `Vec<LuaCommand>` batches through bounded channels. This is the "event-driven state machine": hooks are small Lua programs that react to compositor events and *emit commands*; they never touch compositor memory directly.
- **A fresh sandbox per evaluation.** Each reload/event evaluation creates a fresh `Lua` (`mlua` `lua54`+`vendored`), installs the prelude, and runs the chunk. This is what makes the plan's "fresh state per parse avoids Send issues" true in practice and prevents stale globals / cross-reload leakage. The *VM thread* is long-lived; the *VM state* is not.
- **Two accepted top-level forms**, transparently reconciled:
  1. **Data-table** (plan-compatible): `return { binds = {...}, layout = {...}, ... }`.
  2. **Imperative** (this roadmap's API): calls to `ymir.bind(...)`, `ymir.action.*`, `ymir.window_rule(...)`, `ymir.on(...)`, with an optional trailing `return { ... }` merged on top for static settings.
- **Canonical path `~/.config/ymir/init.lua`.** `config.lua` is rejected with a clear error pointing to `init.lua` (no silent legacy fallback). All packaging/docs/tests move to `.lua`.
- **`notify` replaces polling for triggers, polling stays for includes.** `notify::RecommendedWatcher` (inotify) fires reload *triggers* into the existing watcher channel; the existing 500 ms `recv_timeout` loop in `src/utils/watcher.rs` remains as the *authoritative mtime/canonical check* (covers the documented include-set race at `watcher.rs:100-104`) and also as an NFS/edge fallback. No double-parse: the notify event only *wakes* the loop, the loop still re-checks `Props` itself.
- **Zero-downtime swap is diff-based** (mirrors today's `reload_config`): non-invasive diff of the new `ConfigProgram` against the live one; changed `Action`s are re-registered (and now **cancel in-flight repeats/cooldowns per I1**); changed springs/colors/gaps propagate through the existing `reload_config` fan-out (`xkb`, libinput, outputs, rules, shaders, cursor, MRU, xwayland-satellite, clock rate). Active surfaces/layouts stay untouched.

#### 2.1 Engine + prelude (`ymir-config`)

1. `ymir-config/Cargo.toml`: add `mlua = { version = "...", features = ["lua54", "vendored", "send"] }`; remove `knuffel`; drop the root `[profile.release.package.ymir-config]` debug override.
2. New `ymir-config/src/lua.rs`: VM-thread supervisor, sandbox construction, prelude:
   - `include_config("path")` — load + deep-merge another config table, `~` expansion, recursion stack (error on self-include) and `RECURSION_LIMIT` (mirror `lib.rs:297-446`).
   - Tracked `dofile`/`require`/`loadfile` recording absolute paths into the include set fed to the watcher (mirror `ConfigParseResult { config, includes }` shape).
   - `ymir.*` API table (see 2.3), `error` handling: record *path even on failure* so a broken include still gets watched and fixed-on-save.
3. New `ymir-config/src/error.rs` rework: `ConfigError` (Lua runtime → traceback text; validation → `section.key: msg`), `ConfigIncludeError` now wrapping it; `ConfigParseResult` shape preserved so `src/utils/watcher.rs` stays near-unchanged.
4. `Config::parse/parse_mem/load` execute the Lua program; empty / `return nil` ⇒ defaults; a chunk that returns neither a table nor calls any `ymir.*` function is an error.

#### 2.2 Section appliers (keep runtime types, replace decoders)

Mechanical, from `FUTURELUAPLAN.md` Phase 2, corrected against Part A.5:

- Shared typed readers (`read_string`, `read_bool`, `read_range`, `read_enum`, `read_color`, `read_list`, `read_optional`) with error accumulation; unknown/duplicate keys gather `section.key` diagnostics instead of fail-fast.
- One `apply_*` per section: `input`, `output`, `layout`, `appearance` (focus-ring/border/shadow/tab-indicator/insert-hint/struts/colors/gradients/blur), `animations`, `gestures`, `misc`, `debug`, `workspace`, `window_rule`, `layer_rule`, `binds`, `recent_windows`, `switch_events` — including the schema keys the old plan omitted (`insert-hint`, `preset-window-heights`, `config-notification`, `overview`, `xwayland-satellite`, `switch-events`, `debug`, `gestures`).
- **Decode-tagged runtime hold types get Lua constructors** (not deletion): `Output`, `Workspace`, `WindowRule`/`LayerRule`/`Match`, `SwitchBinds`/`SwitchAction`, `Input` sub-types, `Environment`, `HotCorners`, `Mru*Part`. Deleted: the `*Part` decode-only types, `macros.rs` merge macros (except the runtime-only `MergeWith` impls §A.5), `expect_only_children`/`parse_arg_node`.
- Preserve exactly: binds replace-by-key; `layout { border {} }` enable / `border.off` disable; numeric workspace layout tags; `Percent`/`FloatOrInt`/`Flag` via existing `FromStr`; `XF86ScreenSaver`; `SawMruBinds` → superseded by deterministic user tables.

#### 2.3 The imperative `ymir` API (this roadmap's extension)

```lua
-- ~/.config/ymir/init.lua
ymir.bind("Mod+Shift+D", function() ymir.action.cycle_display() end)
ymir.bind("Mod+J", function() ymir.action.focus_down() end, { repeat = true, allow_when_locked = false })
ymir.set_layout_defaults {
  mode = "dwindle",           -- "dwindle" | "scrollable" | "depth" | "tabbed"
  gaps = 16,
  border = { width = 3, color = { r=255, g=200, b=100, a=255 }, gradient = "oklab" },
  animations = { focus_shuffle = { spring = { damping_ratio = 0.7, stiffness = 800, epsilon = 0.0001 } } },
}

-- Window / layer rules as pure-Lua matchers (safe: they only return data)
ymir.window_rule(function(w) return w.app_id:match(".*alacritty") end)
  :apply { open_on_output = "eDP-1", default_column_display = "tabbed" }
ymir.layer_rule(function(l) return l.namespace == "^notifications$" end)
  :apply { block_out_from = "screencast" }

-- Event-driven extensions (the state-machine half); each returns a command table
ymir.on("window.new", function(w) if w.app_id == "mpv" then return { action = "set-column-display", arg = "depth" } end end)
```

Implementation notes:

- **`ymir.bind`** compiles the human-readable key string with the existing `Key::from_str`/`Trigger` parser; the handler `function` is not stored across reloads (the sandbox is per-eval) — instead the *outcome* (the resolved `Action` or a small Lua-command enum) is captured into the program. Action names resolve against the same `Action`/`OutputAction` enums used by IPC (`ymir-config/src/binds.rs:151` ↔ `ymir-ipc`), so keymaps, config and `ymir msg` share one vocabulary.
- **`ymir.window_rule(lua-fn)`** registers a matcher *function*. Because the VM is thread-local and recreated per evaluation, matchers are evaluated inside the runtime thread. The compositor never blocks on it: window-open/close/focus events are batched to the thread; the thread runs affected hooks with a bounded Lua instruction budget (`debug.sethook`, reaped after an allocation/instruction cap + wall-clock deadline); results are returned as `LuaCommand`s. This keeps user scripts (even buggy/infinite ones) from stalling the render loop — they only stall their own thread and are then killed.
- **Safety surface:** the prelude exposes only data constructors + the high-level actions; no `io`/`os`/`ffi` and no access to compositor memory. `spawn` is the only escape hatch to a subprocess, mirroring `spawn_at_startup` semantics.

#### 2.4 Hot reload + event plumbing

1. **Watcher upgrade** (`src/utils/watcher.rs`): add `notify` as a trigger; keep `Props`/`check()`/`set_includes`; keep parsing off-loop; forward `Result<ConfigProgram,()>` through the existing bounded channel; keep the failure UX (`config_error_notification.rs`) — now also reporting Lua tracebacks.
2. **Reload specifics:** solve I1 (cancel repeat/cooldown for changed keys) *and* I2 (reset scroll trackers) in `reload_config`; re-register rules/hooks without rebuilding the layout; re-run `hotkey_overlay`/`switch` display-cycle matrices if `ColumnDisplay` set membership changed (Phase 3 makes the cycle configurable); propagate new gradient/blur/spring parameters as in 2.0.
3. **Timeout/runaway policy:** instruction-count + wall-clock budget per hook; runaway scripts are aborted with a `ConfigError`, config kept, notification shown; the watcher keeps watching so a fix reloads on next save (same contract as broken includes today).
4. **`ymir validate`** (`src/cli.rs`): executes the program (settings + binds + a dry-run of every registered rule against a sample window), reports Lua tracebacks and `section.key` diagnostics; exit code non-zero on error — no restart required.

#### 2.5 Migration sweep

Per Part A.5 blast radius, all in one change set (or split as: engine → appliers → sweep → tests):

- `resources/default-config.kdl`/`dwindle-config.kdl` → `resources/default-config.lua`/`dwindle-config.lua` (settings preserved 1:1; dwindle shipped as the default `ymir.set_layout_defaults { mode = "dwindle" }`).
- `src/main.rs:345,350`, `src/cli.rs:16,49`, `ymir-config/src/lib.rs` doc/path strings → `init.lua`.
- `ymir.spec.rpkg:157`, `scripts/install.sh:10,22,241-245`, `README.md:105,116,136`, `PKGBUILD` (vendored Lua C-toolchain note: ensure a C compiler is listed in makedepends and `CC`/`CFLAGS` propagate to the vendored `liblua` build).
- Tests: `src/utils/watcher.rs`, `src/ui/hotkey_overlay.rs`, `src/tests/window_opening.rs`, `src/tests/floating.rs`, **`src/layout/tests.rs:2364,2447`**, all `ymir-config` KDL-snippet tests, and a rewrite of `tests/wiki-parses.rs` to validate the shipped `.lua` files (must-fail → must-succeed semantics).
- `docs/wiki/*`: the 17 `Configuration:*` pages + `Getting-Started`/`Integrating-ymir`/`Packaging-ymir` (Phase 5 of the original plan; fold into this phase since it's now the only remaining surface).

**Exit criteria:** zero `kdl`/`knuffel` matches in the workspace (except this roadmap); `ymir validate` + `cargo test --workspace` green; manual checklist: (1) edit `init.lua` → keymap + style + default-mode update without dropping focus or a single surface; (2) change an included `colors.lua` → hot-reload; (3) break a rule function → notification + prior config retained, fix-on-save reload; (4) user hook with `while true do end` → runtime kills it, compositor stays responsive.

---

### Phase 3 — Depth-Queue Mode (`depth`)

**Goal:** a serious, production-grade third display mode. A column becomes a physical stack of interactive paper cards with perspective depth-sorting: the **focused card at the apex** (full width, fully opaque, interactive), the rest of the queue fanned into a stacked deck behind the top and bottom edges (decreasing opacity + hardware-accelerated background blur). Focus navigation spring-animates the shuffle. Fully exposed to config, keybindings and `ymir msg`.

> The mode name in code/branding is `depth`; docs may refer to it as "depth-queue". It coexists with `dwindle`/`normal`/`tabbed` and joins the `ColumnDisplay` cycle.

#### 3.1 Data model

1. **Enum.** `ColumnDisplay::Depth` added at `ymir-ipc/src/lib.rs:1019-1026` (with `FromStr`/serde/clap wiring) alongside `Normal | Tabbed | Dwindle`.
2. **`DepthState`** on `Column` (`layout/scrolling.rs:167-250`), parallel to `dwindle_tree`/`tab_indicator`:
   ```rust
   pub struct DepthState {
       /// Ordered queue, same length/invariant direction as `Self::tiles` (apex = index 0).
       queue: Vec<DepthCard>,        // index maps to tiles[] via the display-mode router
       active: usize,                // == column.active_tile_idx (kept in sync)
       /// Current animation-time geometry of every card (spring driven, per card).
       anim: Vec<DepthCardAnim>,     // offset_y, scale, tilt, opacity, blur — see 3.3
       /// The apex trace of the last shuffle, for chaining springs without teleports.
       last_apex: Option<DepthCardAnim>,
       /// Spring params resolved from `options.animations.depth_shuffle`.
       spring: Spring,
   }
   pub struct DepthCard { pub tile_idx: usize, pub side: DeckSide }        // Top/Bottom deck
   pub struct DepthCardAnim {
       pub offset_y: f64,   // logical, deck slot position
       pub scale_y: f64,    // perspective squash
       pub rot_x: f32,      // fan tilt around the far edge (radians)
       pub opacity: f32,    // 1.0 apex → configurable floor at the far deck
       pub blur: f32,       // mounted backdrop-blur radius under the deck region
   }
   ```
   `queue` is kept in sync with `tiles[]` by the same invariant discipline as the dwindle tree (`reorder_tiles_by_dfs` ancestry) — Phase 1.1-B4 hardened this class of bug first.
3. **Config surface** (Phase 2 API + KDL-era fields replaced):
   ```lua
   layout = {
     mode = "depth",
     depth_queue = {
       card_height_ratio = 0.62,   -- apex card height / column height
       top_deck_size  = 2,   bottom_deck_size = 2,     -- cards visible in each fan
       gap = 12, deck_bleed = 24,                        -- physical overlap into margins
       min_opacity = 0.35,  blur_radius = 18, card_shadow = {offset={x=0,y=10}, blur=24},
       perspective_tilt = 7.0,                            -- degrees, near-edge fan
       focus_shuffle = { spring = { damping_ratio = 0.62, stiffness = 750, epsilon = 0.0001 } },
     },
   }
   ```

#### 3.2 Geometry & layout algorithm

Runs inside the display-mode router (Phase 1.4 item 3) whenever the workspace is laid out (`ref_region`/`tile_height` distribution path in `scrolling.rs`):

1. **Apex card.** Height = `card_height_ratio × working_area.height`, width = `resolve_column_width(Proportion(1.))` (dwindle-style full column width), vertically centered in the working area.
2. **Top deck.** Cards preceding the apex (`queue[1..=top_deck_size]`), stacked upward from the apex's top edge: each successive card overlaps the previous by `deck_bleed`, offset by `-deck_step` where `deck_step` grows along the fan; each is squashed (`scale_y < 1`), tilted around its bottom edge by `rot_x` such that the fan "flares" toward the top edge, and clamped inside the top margin (`air`), never into the apex card's content region.
3. **Bottom deck.** Mirror-image: cards succeeding the apex stack downward with the same squash/tilt toward the bottom edge; they occlude the apex only at its margin strip (see 3.3 clipping).
4. **Queue order** is the column's existing logical order (same as scrolling order); `focus_down`/`focus_up` move the active index and re-fan the decks (the "shuffle").
5. **Minimum sizes.** The apex card honors the window's `min_size` by growing the card (never below `card_height_ratio`, bounded by working area); deck cards are clipped presentation previews — the *live* interactive surface is always the apex card. Windows sized/positioned off-apex never receive input, never configure below min-size, and are only ask-queued for geometry (see 3.6).
6. **Fractional-scale integration:** all card rects computed in `f64` logical then rounded via the existing `to_physical_precise_round` discipline (Part A scale audit R8); the apex content surface uses the standard surface/texture render path unchanged.

#### 3.3 Rendering & shaders

New files under `src/render_helpers/shaders/`, compiled centrally (`#version 100`, uniform conventions per `shader_element.rs`):

- **`depth_card.vert` / `depth_card.frag`** (a new `ShaderRenderElement` variant or a subclass of `BorderRenderElement`): applies per-card `offset_y` + `scale_y` + `rot_x` as a pseudo-3D matrix, a vertical alpha gradient from the far edge toward the near edge (falloff couples to `min_opacity`), and, when `blur > 0`, composites the **mounted backdrop-blur texture** behind the card with the card's own surface sampled from the standard texture path as an alpha-masked overlay. Uses the existing `rounding_alpha` approach for card corners. Premultiplied alpha — not straight — and reuses the fixed sRGB EOTF / gamut-safe helpers landed in Phase 1.2.
- **Backdrop mount for the deck:** render the blurred backdrop *behind* the deck fan, not per-card, using the existing `BackgroundEffectElement`/`Blur` pipeline mounted on a deck-region element (union of the fanned rects). The far deck blur reduces exactly to the blur texture produced by the mounted region (Phase 1.2 item 5 makes this cheap during animations).
- **Card shadows:** reuse `render_helpers/shadow.rs` with the depth-deck offset config so decks read as physically stacked paper.
- **GPU budget (design target):** at rest (no animation in flight), zero card pass cost beyond a static deck texture; during a shuffle, exactly one extra offscreen pass for the deck + one for the apex composite, matching the existing resize/close crossfade cost. Reuse the blur mip reuse work from Phase 1.2 to avoid per-frame reallocation during the shuffle. Damage is the union of the two deck rects + apex, computed from the same anim params that drive the rails.

#### 3.4 Focus transition state machine ("the shuffle")

1. **Triggers.** `focus_down`/`focus_up` (and IPC equivalents `cycle-queue-depth`, §3.5) target the next card in the queue; `promote`/`pull-to-apex` promote an arbitrary card to the apex (analog of dwindle's `promote-window`); mouse click on a deck card pulls it to the apex (`pick_window` gradient).
2. **Animation.** On activation, the departing apex card's `DepthCardAnim` is captured into `last_apex`, all affected cards' goal states are recomputed, and a single `Spring` per degree of freedom per card is (re)started from its *current* anim state — never from a set goal — so a rapid chain of `focus_down` inputs mid-flight queues the next shuffle without a teleport or a snap (Phase 1.2's spring `duration()` fix guarantees clean termination). The shuffle lasts `spring.duration()`; while running, the active card is the *animation target* for input, the apex content which completes fastest (scale/opacity rails lead, offset rails lag ~10-15% for the "settle" feel).
3. **Focus semantics during the shuffle.** Keyboard focus moves on frame zero (snappy, matches niri/ymir input latency); the *visual* apex lands when the spring converges. Screencast/accessibility always report the *focused* window, not the visual one.
4. **Static-hygiene:** no frame callbacks when the spring converged and no damage is pending (reuse of a `needs_frame` gate, mirroring the existing animation clock discipline).

#### 3.5 IPC & action surface

New actions (all three layers: `ymir-ipc/src/lib.rs` enum, config ↔ IPC conversion at `ymir-config/src/binds.rs`, dispatch at `input/mod.rs`):

- `SwitchColumnDisplay` cycle matrix extended and made config-orderable: `[normal, dwindle, depth, tabbed]` with a config subset (e.g. only `["dwindle", "depth"]`).
- `SetColumnDisplay { display: Depth }`, per-column and per-workspace (works like `set-column-display-at`).
- `FocusCardUp` / `FocusCardDown` — aliases mapping to existing `focus-up`/`focus-down` semantics inside depth columns.
- `PushToQueue` (`push-to-queue`) — move focused window to the far end of the queue (dwindle-style `consume`).
- `PullToApex` (`pull-to-apex`) — promote focused window (or by-id arg) to the apex immediately (springs the shuffle).
- `CycleQueueDepth` (`cycle-queue-depth`) — Alt+Tab-style live cycler through the deck with previews, mirroring `recent_windows` integration.
- `Cover`/`ShowQueue` (`cover-all`) optional: span the deck fanned so N cards are all partly visible — the "serious productivity" multi-view.

All of the above appear in `ymir msg <action>` via the existing clap/serde derivation (`ymir-ipc` feature `clap`, `ipc/server.rs` `validate_action`), bindable through `ymir.bind(...)` in Phase 2, and reported in `ymir msg windows`/`workspaces` (`WindowLayout`/`ColumnDisplay` serialization).

#### 3.6 Mode conversion & edge cases

- **→ depth:** from `normal`, the queue order preserves column order; from `dwindle`, `DwindleTree::leaf_paths()` yields the canonical order (or the *visual* DFS order via `leaf_rects` sort) which becomes the queue; from `tabbed`, tab order becomes queue order. All non-apex windows are ask-queued to a reduced animation frame (their real size only matters at the apex) and the scroll offset resets.
- **depth → any:** the last apex window becomes the active tile in the target mode; deck preview cards restore their real sizes through the standard tile-size animation path (`RESIZE_ANIMATION_THRESHOLD` gate applies).
- **Fullscreen/maximize:** apex card fullscreens normally (deck hidden for the fullscreen duration); un-fullscreen restores the deck. `is_pending_maximized` behaves as today.
- **Close of apex:** the next `focus` target (MRU-correct) becomes apex; the fan re-flows. **Mid-shuffle close** resolves against `DepthState.anim` slots which are lifetime-tracked (Phase 1.1 hardening reused) — no dangling `tile_idx`.
- **Window taller than the apex slot:** clamped per 3.2-5 (never below `min_size`); overflow never configures off-apex windows below min size.

**Exit criteria:** `ymir msg` + keybinding drive `push-to-queue` / `pull-to-apex` / `cycle-queue-depth`; a 120-frame screen recording of a rapid `focus_down × 4` chain shows no teleport/snap and ends pixel-stable; deck blur, shadows and fan tilt render on both llvmpipe and NVIDIA; no frame callbacks burn when the deck is static; `switch-column-display` cycles `depth` in both orders; `depth` survives output hotplug and workspace move (Phase 4 tests).

---

### Phase 4 — Verification, Testing & Polish

#### 4.1 Test matrix

| Level | What | Where |
|-------|------|-------|
| Unit | Spring `duration()` ⇒ `|value−to| ≤ ε` for r ∈ {0.3,0.6,0.8,1.0}; sRGB EOTF vs. table; Oklab/oklch gamut-output finiteness; `DepthState` re-fan after close/move; `DwindleTree` invariant fuzz | `animation/spring.rs`, `render_helpers`, `layout/dwindle.rs` + `tests` |
| Layout ops | Depth: open/focus/close/reorder/mode-conv ↔ `normal`/`dwindle`/`tabbed`; hotplug (add/remove/primary-change); sticky empty workspace invariant under `NoOutputs⇄Outputs` | `src/layout/tests.rs` (extend the existing op-script harness; **this was the missing migration file in the Lua plan** — keep it, add Depth ops) |
| Integration | Grab-safety: kill window mid move/resize/tab-cycle (B1/B5); maximize/add-window via IPC with stale ids (B2/B3); Lua watcher reload suite (hot-swap, broken include, runaway hook) | `src/tests/*`, `src/utils/watcher.rs` |
| Client | Wayland client end-to-end for depth apex/deck input routing and screencast-opacity correctness | `src/tests/client.rs`, `ymir-visual-tests` |
| Visual | Gradient borders (oklab/oklch chords incl. out-of-gamut), blur corners, depth deck at 1.5× scale, mixed-DPI span | `ymir-visual-tests` + screenshot diffs |
| Static gates | `cargo test --workspace` · `cargo clippy --workspace --all-targets -- -D warnings` · `cargo fmt --check` · the CI panic-capable-call grep (re-gate: no `unwrap()`/`expect(` on pointer-motion/key/commit/hotplug paths) | CI / `scripts/` |

#### 4.2 Performance & GPU budget

- Per-frame CPU: layout unchanged O(n) per column; depth adds one O(deck) rect computation per damage.
- GPU: static deck = cached texture; shuffle = one mounted-blur pass + one apex pass; **no per-frame mip reallocation** (Phase 1.2 item 5); no frame callbacks when converged.
- Memory: deck textures are department-of-the-mounted-region; blob budgets as today.
- Add a `--list-frames`-style debug print (or reuse `Debug::PreviewRender`) to assert the no-static-wakeup property manually and via a CI smoke test.

#### 4.3 Docs & packaging

- `README.md`: drop `config.kdl` (:.105,116,136); document `~/.config/ymir/init.lua`, the `ymir` API, depth-queue mode + its IPC, and the mode cycle; add a "Third display mode: Depth-Queue" section and update "Three column display modes" wording to four (`depth`, `dwindle`, `normal`, `tabbed`).
- Wiki: rewrite all `Configuration:*` pages to Lua; new `Configuration:-Depth-Queue.md`; update `IPC.md`, `Development:-Design-Principles.md`, `Getting-Started.md`, `Integrating-ymir.md`.
- Packaging: `ymir.spec.rpkg:157` (`%doc resources/default-config.lua`), `scripts/install.sh` seeds `init.lua` from `Dwindle` default, `PKGBUILD` adds the C toolchain needed by vendored Lua, `ymir.desktop`/service names unchanged (already clean).
- Remove residual niri metadata links (§A.6) and `FUTURELUAPLAN.md` → superseded by this roadmap (or fold its "Open decisions" into the Phase 2 ticket).

#### 4.4 Release checklist

1. Full test matrix green (4.1).
2. Manual smoke on Mesa/llvmpipe **and** NVIDIA with mixed-DPI (1.0 / 1.25 / 1.5 / 2.0) and all four column modes incl. depth.
3. `ymir validate` on a fresh user `init.lua`; error-notification path exercised.
4. Hot-reload exercised end-to-end (main file, included file, broken file, runaway hook).
5. Panic-capable-call grep clean on hot paths; Nix flake + installer + rpm/deb + dinit/systemd paths re-verified.
6. Bump package metadata and cut a tagged release per `docs/wiki/Development:-Releasing-ymir.md`.

---

## Appendix — Key reference locations

- Column display surface: `ymir-ipc/src/lib.rs:1019-1026` (`ColumnDisplay`), `src/layout/scrolling.rs:211,2431-2482,5886` (router + cycles), `src/layout/mod.rs:2355`, `src/input/mod.rs:1646-1663`.
- Dwindle engine: `src/layout/dwindle.rs` (tree), `src/layout/scrolling.rs:1230-1279,4751,6180-6187` (tiles↔tree wiring).
- Config: `ymir-config/src/lib.rs` (+ section modules), `src/utils/watcher.rs`, `src/ymir.rs:1440-1642` (`reload_config`), `src/ui/config_error_notification.rs`.
- Render: `src/render_helpers/shaders/border.frag` (oklab/oklch), `blur*.frag` + `src/render_helpers/blur.rs`, `src/render_helpers/shader_element.rs:380-420` (texture wrap), `src/animation/spring.rs`, `src/render_helpers/background_effect.rs`.
- Test harness: `src/layout/tests.rs`, `src/tests/*`, `ymir-visual-tests`, `ymir-config/tests/wiki-parses.rs`.