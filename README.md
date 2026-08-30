<h1 align="center">ymir</h1>
<p align="center">A scrollable-tiling Wayland compositor with dwindle-style binary tiling.</p>
<p align="center">
    <a href="https://lab.braxton.onl/braxton/ymir/src/branch/main/LICENSE"><img alt="License" src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue"></a>
    <a href="https://lab.braxton.onl/braxton/ymir/releases"><img alt="Release" src="https://img.shields.io/badge/release-1.0.0-blue"></a>
    <img alt="Platform" src="https://img.shields.io/badge/platform-Linux-lightgrey">
</p>

## What is ymir?

ymir is a Wayland compositor built around **scrollable tiling**: windows are arranged in
columns on an infinite strip that scrolls to the right, and opening a window never disturbs the
windows you already have open. It is named after Ymir from Norse mythology, and is written in
Rust on top of [Smithay].

Its signature feature is **Dwindle**, a binary-split tiling layout that is **on by default**:
inside a column, each new window splits the focused window in two and takes half of its space.
The result is a resizable binary tree that partitions the whole work area — Hyprland-style,
living inside a scrollable column strip.

Four column display modes are available — `dwindle`, `normal` (classic
scrollable columns), `tabbed` and `depth` (a deck-style queue with a full-size
apex card) — and you can switch a column between them at any time.

ymir began as a fork of [niri] and has grown its own tiling model and identity since.

## How tiling works

- **Columns scroll, nothing resizes.** Windows live in vertical columns on an infinite
  horizontal strip. Every monitor has its own strip, and windows never overflow onto an
  adjacent monitor. Opening a new scrollable-column window never resizes the existing ones.
- **Dwindle splits the focused window.** In a dwindle column, a new window halves the focused
  window and takes the freed-up space. By default wide regions split side-by-side (the new
  window lands to the right) and tall or square regions stack (the new window lands at the
  bottom); you can preselect a fixed direction beforehand. Splits start at 50/50.
- **Divider drags are clamped.** Every split is a draggable divider that respects each window's
  minimum size, so a resize can never crush a window.
- **Workspaces are dynamic.** Workspaces are arranged vertically, GNOME-style: every monitor
  has an independent set, and one empty workspace is always present at the bottom. The
  arrangement survives monitor hotplugging — disconnect a monitor and its workspaces move
  along, reconnect and they come back.

## Highlights

- **Dwindle-first.** The shipped default config boots straight into dwindle. Classic scrollable
  tiling and tabbed columns are one `Mod+Shift+D` away, per column, at runtime.
- **Per-workspace layout config.** Name a workspace `"1"` (numeric) and give it a `layout {}`
  block, and ymir applies it to the Nth default workspace of every monitor *without creating a
  named workspace*. Non-numeric names (`"browser"`) still create real named workspaces. See
  [Configuration: Named Workspaces](docs/wiki/Configuration:-Named-Workspaces.md).
- **Everything modern.** An Overview zoom, built-in screenshot UI, screencasting through
  xdg-desktop-portal, touchpad and mouse gestures plus hot corners, gradient borders (Oklab and
  Oklch), background blur, spring animations with custom shaders, and live-reloading config.

## Features

- Scrollable tiling built from the ground up
- [Dwindle column mode](docs/wiki/Configuration:-Dwindle.md): resizable binary-split tree,
  draggable dividers, split preselection, spatial window moves
- Tabbed and normal (scrollable) column displays, switchable at runtime
- [Dynamic workspaces](docs/wiki/Workspaces.md) like in GNOME, one empty workspace always at the
  bottom, preserved across hotplugs
- [Overview](docs/wiki/Overview.md) that zooms out workspaces and windows; recent-windows
  (Alt+Tab-style) switcher with live previews
- Built-in screenshot UI, screenshot of screen and of focused window
- Monitor and window [screencasting](docs/wiki/Screencasting.md) through xdg-desktop-portal,
  with [block-out rules](docs/wiki/Configuration:-Window-Rules.md)
- Touchpad and mouse [gestures](docs/wiki/Gestures.md) and [hot corners](docs/wiki/Configuration:-Outputs.md):
  overview, workspace switch, column moves, resize and column swipe
- Configurable layout: gaps, focus ring, borders and shadows, struts, preset column widths and
  window heights
- [Gradient borders](docs/wiki/Configuration:-Layout.md) with Oklab and Oklch support, plus
  insert hints
- [Background blur](docs/wiki/Window-Effects.md) for windows and layer-shell surfaces
- [Animations](docs/wiki/Configuration:-Animations.md) with configurable springs and curves and
  support for custom shaders
- Per-workspace and per-output layout overrides, plus named-workspace sticky focus
- Fullscreen (with standalone toggle), maximize, floating windows
- Lives on the [wlr layer-shell](docs/wiki/Layer‐Shell-Components.md), plus gamma-control,
  screencopy, output-management, foreign-toplevel and ext-workspace protocols
- Xwayland through xwayland-satellite, screen-reader support (accesskit/dbus), systemd and
  D-Bus integration, keyboard-shortcuts-inhibit
- Live-reloading config and full IPC control via `ymir msg`

## Getting Started

The full instructions are on the [Getting Started](docs/wiki/Getting-Started.md) wiki page.
ymir is not a complete desktop environment: grab a status bar like [waybar], and adjust the
config to spawn your own terminal and launcher — the default config expects [alacritty] and
[fuzzel].

### Linux (installer)

A multi-distro installer detects your distro (Arch, Fedora, Debian/Ubuntu, openSUSE), installs
the build/runtime dependencies, clones the latest `main`, builds and installs:

```sh
# bash/zsh
curl -sL https://lab.braxton.onl/braxton/ymir/raw/branch/main/scripts/install.sh | bash

# fish
curl -sL https://lab.braxton.onl/braxton/ymir/raw/branch/main/scripts/install.fish | fish
```

On Arch it builds with `makepkg` from the bundled PKGBUILD; elsewhere it builds with `cargo`
and installs into `/usr/local`. It seeds `~/.config/ymir/init.lua` with the dwindle example
config if absent, and installs the `ymir.desktop` session entry so "Ymir" appears in your login
manager (GDM, SDDM, ...). From a bare TTY you can start it with `ymir-session`. Re-running the
installer pulls the latest `main` and rebuilds, so it doubles as an updater.

### From source

```sh
cargo build --release
```

On first start, ymir creates a config at `~/.config/ymir/init.lua` based on the embedded
default config (which enables dwindle). Start it with `target/release/ymir --session` from an
unlocked TTY. Development shells are also provided via `nix develop`.

## Dwindle column mode

Dwindle is ymir's default layout mode. When a window opens in a dwindle column, the focused
window's region is split in two: the focused window keeps its corner and shrinks, while the new
window takes the freed-up half. Dividers are draggable and clamped to each window's minimum
size.

| Key combo | Action |
| --- | --- |
| `Mod+Shift+D` | switch-column-display: cycle the focused column between dwindle, normal (scrollable), tabbed and depth |
| `Mod+Space` | toggle-split: flip the split orientation of the focused window's container |
| `Mod+Ctrl+Space` | preselect `"bottom"`: split direction for the next window opened in the column |
| `Mod+Shift+Home` | promote-window: move the focused window to the head of the dwindle tree |
| `Mod+Shift+Left` / `Mod+Shift+Right` (also `H`/`L`) | move-window-left/right: swap the focused window with its spatial neighbor |
| `Mod+Comma` / `Mod+Period` | consume-window-into-column / expel-window-from-column |

See the [default config](resources/default-config.lua) for the full list of bindings.

## Documentation

The wiki has detailed docs on configuration, key bindings, window effects, IPC and
development:

- [Configuration introduction](docs/wiki/Configuration:-Introduction.md)
- [Key bindings](docs/wiki/Configuration:-Key-Bindings.md)
- [Layout configuration](docs/wiki/Configuration:-Layout.md)
- [Dwindle layout](docs/wiki/Configuration:-Dwindle.md)
- [Named workspaces](docs/wiki/Configuration:-Named-Workspaces.md)
- [Window rules](docs/wiki/Configuration:-Window-Rules.md)
- [Recent windows](docs/wiki/Configuration:-Recent-Windows.md)
- [IPC: `ymir msg`](docs/wiki/IPC.md)
- [FAQ](docs/wiki/FAQ.md)

## Status

ymir is stable for day-to-day use and does most things you'd expect of a Wayland compositor.

- **Multi-monitor**: yes, a core part of the design from the start. Mixed DPI works.
- **Fractional scaling**: yes, and ymir UI stays pixel-perfect.
- **NVIDIA**: seems to work fine — see [Nvidia.md](docs/wiki/Nvidia.md).
- **Floating windows**: yes, with a dedicated focus-movement path.
- **Input devices**: tablets, touchpads and touchscreens are supported; tablets can be mapped
  to a monitor or the focused window, and work with [OpenTabletDriver]. Touchpad gestures are
  available, but no touchscreen gestures yet.
- **Wlr protocols**: layer-shell, gamma-control, screencopy, output-management and more.
- **Performance**: development stays conscious of runtime and compile budgets.

## License

ymir is distributed under the GPL-3.0-or-later license. See [LICENSE](LICENSE).

[Smithay]: https://github.com/Smithay/smithay
[niri]: https://github.com/YaLTeR/niri
[waybar]: https://github.com/Alexays/Waybar
[alacritty]: https://github.com/alacritty/alacritty
[fuzzel]: https://codeberg.org/dnkl/fuzzel
[OpenTabletDriver]: https://opentabletdriver.net/