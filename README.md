<h1 align="center">Ymir</h1>
<p align="center">A scrollable-tiling Wayland compositor with dwindle-style binary tiling.</p>
<p align="center">
    <a href="https://github.com/5raxton/ymir/blob/main/LICENSE"><img alt="License" src="https://img.shields.io/github/license/5raxton/ymir"></a>
    <a href="https://github.com/5raxton/ymir/releases"><img alt="Release" src="https://img.shields.io/github/v/release/5raxton/ymir?logo=github"></a>
    <img alt="Platform" src="https://img.shields.io/badge/platform-Linux-lightgrey">
</p>

## About

Ymir arranges windows in columns on an infinite strip going to the right, and tiles them
*binary-tree-style* inside those columns. Two tiling ideas, one compositor — the best of
both worlds.

- **Scrollable tiling.** Windows live in a column strip; a new window never resizes the ones
  you already have open. Every monitor has its own strip, and windows can never overflow onto
  an adjacent monitor.
- **Dwindle tiling.** Inside a column, each new window splits its focused neighbor in two and
  takes half of the freed-up space. The result is a resizable binary tree that partitions the
  whole work area — Hyprland-style, inside a scrollable container.

Workspaces are dynamic and arranged vertically, GNOME-style: every monitor has an independent
set, with one empty workspace always present at the bottom. The arrangement survives
monitor hotplugs — disconnect and the workspaces move along, reconnect and they come back.

Ymir began life as a fork of [niri] and has since grown its own tiling model and identity —
the [PaperWM]-inspired scrollable strip as its foundation, with a dwindle binary tree as its
heart.

## Why Ymir

- **Dwindle-first by default.** New installs boot straight into the dwindle layout — no
  assembly required.
- **Resizable dividers.** Every split is draggable and respects each window's minimum size.
- **Two layouts, one keystroke.** Toggle any column between dwindle and classic scrollable
  tiling at runtime with `Mod+Shift+D`. Place a window fleetingly in dwindle and promote it
  out — no restart, no redesign.
- **Per-workspace layout config.** Rule yourself a dwindle-first desktop without a named
  workspace: `workspace "1" { layout { default-column-display "dwindle" } }` targets the Nth
  default workspace directly. See [Configuration: Named Workspaces](docs/wiki/Configuration:-Named-Workspaces.md).

## Features

- Scrollable tiling built from the ground up
- [Dwindle column mode](docs/wiki/Configuration:-Dwindle.md) with draggable split dividers
- [Dynamic workspaces](docs/wiki/Workspaces.md) like in GNOME, one empty one always at the bottom
- An [Overview](docs/wiki/Overview.md) that zooms out workspaces and windows
- Built-in screenshot UI
- Monitor and window [screencasting](docs/wiki/Screencasting.md) through xdg-desktop-portal,
  with a dynamic cast target and [block-out rules](docs/wiki/Configuration:-Window-Rules.md)
  for sensitive windows
- Touchpad and mouse [gestures](docs/wiki/Gestures.md): overview, workspace switch,
  move-column-to-workspace, resize-column, and column-swipe
- Group windows into [tabs](docs/wiki/Tabs.md)
- Configurable layout: gaps, borders, struts, window sizes
- [Gradient borders](docs/wiki/Configuration:-Layout.md) with Oklab and Oklch support
- [Background blur](docs/wiki/Window-Effects.md) for windows and layer-shell surfaces
- [Animations](docs/wiki/Configuration:-Animations.md) with support for custom shaders
- Per-workspace layout overrides via [named](docs/wiki/Configuration:-Named-Workspaces.md)
  and numeric default-workspace config tags
- Live-reloading config
- Works with [screen readers](docs/wiki/Accessibility.md)

## Dwindle column mode

Dwindle is Ymir's default layout, layered on top of the scrollable column strip. When a window
opens, the focused window's region is split in two: the focused window keeps its corner and
shrinks, while the new window takes the freed-up half.

By default, wide regions split side-by-side (the new window lands to the right) and tall or
square regions stack (the new window lands at the bottom); you can also preselect a fixed
split direction beforehand. New splits start at an equal 50/50 size.

Key bindings (see the [default config](resources/default-config.kdl) for the full list):

| Key combo | Action |
| --- | --- |
| `Mod+Shift+D` | switch-column-display: toggle the focused column between dwindle and scrollable layout |
| `Mod+Space` | toggle-split: flip the split orientation of the focused window's container |
| `Mod+Ctrl+Space` | preselect the split direction for the next window opened in the column |
| `Mod+Shift+Home` | promote-window: move the focused window to the head (leftmost) of the dwindle tree |
| `Mod+Shift+Left/Right` (also `H/L`) | move-window-left/right: swap the focused window with its spatial neighbor |
| `Mod+Comma` / `Mod+Period` | consume-window-into-column / expel-window-from-column |

Tabbed and normal (scrollable) column displays remain fully supported, and a column can be
switched between any of them at any time.

## Getting Started

The full instructions live in the [Getting Started](docs/wiki/Getting-Started.md) wiki page.
Ymir by itself is not a complete desktop environment: grab a status bar like [waybar], and
adjust the config to spawn your own terminal and launcher — the default config expects
[alacritty] and [fuzzel].

### Linux

Ymir ships a multi-distro installer script that detects your distro (Arch, Fedora,
Debian/Ubuntu, openSUSE), installs the required build/runtime dependencies, clones the latest
`main`, and builds & installs the compositor:

```sh
# bash/zsh
curl -sL https://raw.githubusercontent.com/5raxton/ymir/main/scripts/install.sh | bash

# fish
curl -sL https://raw.githubusercontent.com/5raxton/ymir/main/scripts/install.fish | fish
```

On Arch it builds with `makepkg` from the PKGBUILD; on other distros it builds with `cargo`
and installs into `/usr/local`. The installer seeds `~/.config/ymir/config.kdl` with the
dwindle example config if absent, and installs the `ymir.desktop` session entry automatically
so "Ymir" appears in your login manager (GDM, SDDM, ...). From a bare TTY you can start it
directly with `ymir-session`.

Re-running the installer pulls the latest `main`, cleans stale build artifacts, and rebuilds —
so it also works as a full update to the bleeding edge.

### From source

```sh
cargo build --release
```

On first start, Ymir creates a config at `~/.config/ymir/config.kdl` based on the embedded
default config. Start it with `target/release/ymir --session` from an unlocked TTY.

The default config enables the dwindle layout out of the box. Set
`default-column-display "normal"` in the `layout` section to switch to classic scrollable
tiling, or toggle it live with `Mod+Shift+D`.

## Documentation

The wiki covers configuration, key bindings, layout, window effects, IPC, and development:

- [Configuration introduction](docs/wiki/Configuration:-Introduction.md)
- [Key bindings](docs/wiki/Configuration:-Key-Bindings.md)
- [Layout configuration](docs/wiki/Configuration:-Layout.md)
- [Dwindle layout](docs/wiki/Configuration:-Dwindle.md)
- [Named workspaces](docs/wiki/Configuration:-Named-Workspaces.md)
- [Window rules](docs/wiki/Configuration:-Window-Rules.md)
- [IPC](docs/wiki/IPC.md)
- [FAQ](docs/wiki/FAQ.md)

## Status

Ymir is stable for day-to-day use and does most things expected of a Wayland compositor.

Things you may be wondering about:

- **Multi-monitor**: yes, a core part of the design from the very start. Mixed DPI works.
- **Fractional scaling**: yes, and all Ymir UI stays pixel-perfect.
- **NVIDIA**: seems to work fine. See [Nvidia.md](docs/wiki/Nvidia.md) for setup.
- **Floating windows**: yes.
- **Input devices**: tablets, touchpads, and touchscreens are supported. You can map a tablet
  to a specific monitor, or use [OpenTabletDriver]. There are touchpad gestures, but no
  touchscreen gestures yet.
- **Wlr protocols**: yes, most of the important ones are implemented — layer-shell,
  gamma-control, screencopy, and more.
- **Performance**: development stays conscious of performance; runtime and compile budgets are
  both kept reasonable.

## Related projects

- [PaperWM] — scrollable tiling on top of GNOME Shell, and the original inspiration.
- [niri] — the scrollable-tiling compositor Ymir forked from.
- [karousel] — scrollable tiling on top of KDE.
- [scroll] and [papersway] — scrollable tiling on top of sway/i3.
- [Paneru] and [PaperWM.spoon] — scrollable tiling on top of macOS.

## Contributing

If you'd like to help with Ymir, there are plenty of both coding- and non-coding-related ways
to do so. See [CONTRIBUTING.md](CONTRIBUTING.md) for an overview.

## License

Ymir is distributed under the GPL-3.0-or-later license. See [LICENSE](LICENSE).

[PaperWM]: https://github.com/paperwm/PaperWM
[niri]: https://github.com/YaLTeR/niri
[waybar]: https://github.com/Alexays/Waybar
[alacritty]: https://github.com/alacritty/alacritty
[fuzzel]: https://codeberg.org/dnkl/fuzzel
[karousel]: https://github.com/peterfajdiga/karousel
[scroll]: https://github.com/dawsers/scroll
[papersway]: https://spwhitton.name/tech/code/papersway/
[Paneru]: https://github.com/karinushka/paneru
[PaperWM.spoon]: https://github.com/mogenson/PaperWM.spoon
[OpenTabletDriver]: https://opentabletdriver.net/