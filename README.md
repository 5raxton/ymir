<h1 align="center">ymir</h1>
<p align="center">A scrollable-tiling Wayland compositor.</p>
<p align="center">
    <a href="https://github.com/5raxton/ymir/blob/main/LICENSE"><img alt="GitHub License" src="https://img.shields.io/github/license/5raxton/ymir"></a>
    <a href="https://github.com/5raxton/ymir/releases"><img alt="GitHub Release" src="https://img.shields.io/github/v/release/5raxton/ymir?logo=github"></a>
</p>

## About

ymir arranges windows in columns on an infinite strip going to the right.
Opening a new window never causes existing windows to resize.

Every monitor has its own separate window strip.
Windows can never "overflow" onto an adjacent monitor.

Workspaces are dynamic and arranged vertically.
Every monitor has an independent set of workspaces, and there's always one empty workspace present all the way down.

The workspace arrangement is preserved across disconnecting and connecting monitors where it makes sense.
When a monitor disconnects, its workspaces will move to another monitor, but upon reconnection they will move back to the original monitor.

## Features

- Built from the ground up for scrollable tiling
- [Dynamic workspaces](docs/wiki/Workspaces.md) like in GNOME
- An [Overview](docs/wiki/Overview.md) that zooms out workspaces and windows
- Built-in screenshot UI
- Monitor and window screencasting through xdg-desktop-portal-gnome
    - You can [block out](docs/wiki/Configuration:-Window-Rules.md) sensitive windows from screencasts
    - [Dynamic cast target](docs/wiki/Screencasting.md) that can change what it shows on the go
- Touchpad and mouse gestures: overview, workspace switch, move-column-to-workspace, resize-column, and column-swipe
- Group windows into [tabs](docs/wiki/Tabs.md)
- Configurable layout: gaps, borders, struts, window sizes
- [Gradient borders](docs/wiki/Configuration:-Layout.md) with Oklab and Oklch support
- [Background blur](docs/wiki/Window-Effects.md) for windows and layer-shell surfaces
- [Animations](docs/wiki/Configuration:-Animations.md) with support for custom shaders
- Live-reloading config
- Works with [screen readers](docs/wiki/Accessibility.md)

### Dwindle column mode

On top of the scrollable layout, ymir ships **Dwindle** as its default layout mode: a binary-split, resizable tiling layout inside the scrollable tiling paradigm.

Dwindle is on by default. New windows split off the focused window into a resizable binary tree.

Dwindle and classic scrollable tiling are fully switchable at runtime: press `Mod+Shift+D` to toggle the focused column between the two layout modes. A ready-made config is also provided at [resources/dwindle-config.kdl](resources/dwindle-config.kdl).

Once in dwindle mode, windows split the current window into two periodically: the focused node keeps half its size and shrinks into its corner, while the newly opened window takes the freed-up half.

Key bindings (see the example config for the full list):

| Key combo | Action |
| --- | --- |
| `Mod+Shift+D` | switch-column-display: toggle the focused column between dwindle and scrollable layout |
| `Mod+Space` | toggle-split: cycle the split direction of the active window (right/up) |
| `Mod+Ctrl+Space` | preselect the split side where the next window will open |
| `Mod+Shift+Home` | promote-window: swap the active node with its sibling's position |
| `Mod+Shift+Left/Right` | move-window-left/right: swap the focused window with its spatial neighbor in the dwindle tree |
| `Mod+Shift+H/L` | move-window-left/right (keyboard-row aliases) |
| `Mod+Comma` | consume-window-into-column |
| `Mod+Period` | expel-window-from-column |

Tabbed and normal (scrollable) column displays remain fully supported, and you can switch a column between them at any time with `set-column-display` / `toggle-column-tabbed-display`.

## Getting Started

The full instructions are in the [Getting Started](docs/wiki/Getting-Started.md) wiki page.
ymir by itself is not a complete desktop environment: grab a status bar like [waybar], and adjust the config to spawn your own terminal and launcher — the default config expects [alacritty] and [fuzzel].

### Linux

ymir ships a multi-distro installer script that detects your distro (Arch, Fedora, Debian/Ubuntu, openSUSE), installs the required build/runtime dependencies, clones the latest `main`, and builds & installs the compositor:

```sh
# bash/zsh
curl -sL https://raw.githubusercontent.com/5raxton/ymir/main/scripts/install.sh | bash

# fish
curl -sL https://raw.githubusercontent.com/5raxton/ymir/main/scripts/install.fish | fish
```

On Arch it builds with `makepkg` from the PKGBUILD; on other distros it builds with `cargo` and installs into `/usr/local`. The installer seeds `~/.config/ymir/config.kdl` with the dwindle example config if absent, and installs the `ymir.desktop` session entry automatically so "Ymir" appears in your login manager (GDM, SDDM, ...). From a bare TTY you can start it directly with `ymir-session`.

Re-running the installer pulls the latest `main`, cleans stale build artifacts, and rebuilds — so it also works as a full update to the bleeding edge.

### From source

Build with cargo:

```sh
cargo build --release
```

On the first start ymir will create a config at `~/.config/ymir/config.kdl` based on the default config. Start it with `target/release/ymir --session` from an unlocked TTY.

The default config enables Dwindle layout out of the box. Set `default-column-display "normal"` in the `layout` section to switch to classic scrollable tiling, or toggle it live with `Mod+Shift+D`.

## Documentation

The wiki contains detailed documentation on configuration, key bindings, window effects, IPC, and development:

- [Configuration introduction](docs/wiki/Configuration:-Introduction.md)
- [Key bindings](docs/wiki/Configuration:-Key-Bindings.md)
- [Layout configuration](docs/wiki/Configuration:-Layout.md)
- [Window rules](docs/wiki/Configuration:-Window-Rules.md)
- [IPC](docs/wiki/IPC.md)
- [FAQ](docs/wiki/FAQ.md)

## Status

ymir is stable for day-to-day use and does most things expected of a Wayland compositor.

Here are some points you may have questions about:

- **Multi-monitor**: yes, a core part of the design from the very start. Mixed DPI works.
- **Fractional scaling**: yes, plus all ymir UI stays pixel-perfect.
- **NVIDIA**: seems to work fine. See [Nvidia.md](docs/wiki/Nvidia.md) for setup.
- **Floating windows**: yes.
- **Input devices**: ymir supports tablets, touchpads, and touchscreens. You can map the tablet to a specific monitor, or use [OpenTabletDriver]. There are touchpad gestures, but no touchscreen gestures yet.
- **Wlr protocols**: yes, most of the important ones are implemented, like layer-shell, gamma-control, and screencopy.
- **Performance**: development stays conscious of performance; runtime and compile budgets are both kept reasonable.

## Inspiration

The scrollable-tiling concept comes from [PaperWM], which implements it on top of GNOME Shell.

## Tile Scrollably Elsewhere

Here are some other projects which implement a similar workflow:

- [PaperWM]: scrollable tiling on top of GNOME Shell.
- [karousel]: scrollable tiling on top of KDE.
- [scroll](https://github.com/dawsers/scroll) and [papersway]: scrollable tiling on top of sway/i3.
- [Paneru] and [PaperWM.spoon]: scrollable tiling on top of macOS.

## Contributing

If you'd like to help with ymir, there are plenty of both coding- and non-coding-related ways to do so.
See [CONTRIBUTING.md](CONTRIBUTING.md) for an overview.

## License

ymir is distributed under the GPL-3.0-or-later license. See [LICENSE](LICENSE).

[PaperWM]: https://github.com/paperwm/PaperWM
[waybar]: https://github.com/Alexays/Waybar
[alacritty]: https://github.com/alacritty/alacritty
[fuzzel]: https://codeberg.org/dnkl/fuzzel
[karousel]: https://github.com/peterfajdiga/karousel
[papersway]: https://spwhitton.name/tech/code/papersway/
[Paneru]: https://github.com/karinushka/paneru
[PaperWM.spoon]: https://github.com/mogenson/PaperWM.spoon
[OpenTabletDriver]: https://opentabletdriver.net/