<div align="center">

<img src="./websrc/logo.png" alt="ymir" width="220" />

# ymir

**A scrollable-tiling Wayland compositor**

</div>

ymir is a Wayland compositor written in Rust on top of [Smithay]. It arranges windows in *columns* across an infinitely-scrolling strip, and ships with **Dwindle**, a binary-split tiling layout that is on by default. Opening a new window never resizes the ones you already have.

- Distributed under the [GPL-3.0-or-later](#license) license.

## Highlights

- **Scrollable tiling with Dwindle.** Windows live in columns on a strip that scrolls to the right. In Dwindle mode every new window splits the focused window in two and takes half of its space, forming a resizable binary-split tree. Switch any column back to classic scrollable tiling at any time with <kbd>Mod</kbd><kbd>Shift</kbd><kbd>D</kbd>.
- **Dynamic workspaces.** GNOME-style workspaces stacked vertically, with one empty workspace always kept at the bottom. Workspaces survive monitor hot-plugging: they remember their original output and migrate back when it reconnects.
- **Modern rendering.** OpenGL ES rendering with damage-based redraws synchronized to VBlank. Includes background and window blur, gradient borders, drop shadows, rounded corners, and fine-grained per-window control of each effect.
- **Lua configuration.** Everything is configured from `~/.config/ymir/init.lua`, with includes, live-reloading via a file watcher, and an imperative `ymir.*` prelude API.
- **Full Wayland feature set.** Layer-shell, decorations, screencasting, screenshots, foreign toplevel and workspace extensions, output management, gamma control, fractional scaling, input methods, session lock, and more.
- **Runs everywhere.** Ships three backends: KMS/DRM on a real TTY, a windowed development mode, and a headless test mode.

## Features

| Area | What ymir does |
| --- | --- |
| Layout | Scrollable tiling, Dwindle binary-split tiling, floating windows, dwindle pages/window-per-column caps, dwindle column widths |
| Windows | Fullscreen, maximize, overview zoom, MRU/Alt-Tab switcher with previews, window rules, focus ring, borders, insert hints, gaps, struts |
| Effects | Window & layer blur, gradient borders, drop shadows, rounded corners, opacity popups |
| Input | Keyboard, mouse, touchpad, touchscreen, tablet, trackpoint & switches via libinput; grabs (click/move/resize/pick); gesture trackers; hot corners; shortcut inhibition; power-off monitors |
| Desktop | Systemd & dinit integration, D-Bus (freedesktop + GNOME/Mutter interfaces), xdg-desktop-portal, system tray/layer-shell bars |
| Accessibility | Full screen-reader support through accesskit + AT-SPI over D-Bus |
| Multi-GPU | Primary render device detection, dmabuf feedback, texture copy between GPUs |
| IPC | Unix-socket JSON API plus the `ymir msg` CLI for full remote control |
| Xwayland | Via Xwayland-satellite, started on demand |

## Getting started

The quickest way to get running is the multi-distro installer (detects Arch, Fedora, Debian/Ubuntu, openSUSE):

```sh
# bash/zsh
curl -sL https://lab.braxton.onl/braxton/ymir/raw/branch/main/scripts/install.sh | bash

# fish
curl -sL https://lab.braxton.onl/braxton/ymir/raw/branch/main/scripts/install.fish | fish
```

The installer seeds `~/.config/ymir/init.lua` with the default Dwindle config if it's absent and installs the `ymir.desktop` session entry so **Ymir** appears in your login manager (GDM, SDDM, …). From a bare TTY, start it with `ymir-session`. Re-running the installer pulls the latest `main` and rebuilds, doubling as a bleeding-edge update.

Once inside:

- <kbd>Super</kbd><kbd>T</kbd> runs a terminal ([Alacritty])
- <kbd>Super</kbd><kbd>D</kbd> runs an app launcher ([fuzzel])
- <kbd>Super</kbd><kbd>Shift</kbd><kbd>E</kbd> exits ymir

The default config assumes Waybar for a status bar and portal/screencast components; see the [wiki] for the full list of [important software] and how to [configure][config-intro] ymir.

> **Note on running inside an existing session:** `ymir` can be opened as a window from inside a desktop environment for a quick try. This windowed mode is mainly for development and can be a little buggy (especially hotkeys).

### Main default hotkeys

When running on a TTY the mod key is <kbd>Super</kbd>; in the windowed development mode it's <kbd>Alt</kbd>. As a rule of thumb, adding <kbd>Ctrl</kbd> to a *switch* hotkey *moves* the focused window or column instead.

| Hotkey | Description |
| --- | --- |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>/</kbd> | Show a list of important ymir hotkeys |
| <kbd>Mod</kbd><kbd>T</kbd> / <kbd>Mod</kbd><kbd>D</kbd> | Spawn `alacritty` / `fuzzel` |
| <kbd>Mod</kbd><kbd>Q</kbd> | Close the focused window |
| <kbd>Mod</kbd><kbd>H</kbd><kbd>L</kbd> or <kbd>←</kbd><kbd>→</kbd> | Focus the column to the left / right |
| <kbd>Mod</kbd><kbd>J</kbd><kbd>K</kbd> or <kbd>↓</kbd><kbd>↑</kbd> | Focus the window below / above in a column |
| <kbd>Mod</kbd><kbd>U</kbd> / <kbd>Mod</kbd><kbd>I</kbd> | Switch to the workspace below / above |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>D</kbd> | Switch the focused column between Dwindle and scrollable tiling |
| <kbd>Mod</kbd><kbd>Space</kbd> | Toggle split orientation of the container (Dwindle) |
| <kbd>Mod</kbd><kbd>M</kbd> | Maximize window |
| <kbd>Mod</kbd><kbd>V</kbd> / <kbd>Mod</kbd><kbd>Shift</kbd><kbd>V</kbd> | Move / switch focus between floating and tiling |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>F</kbd> | Toggle fullscreen |
| <kbd>PrtSc</kbd> / <kbd>Alt</kbd><kbd>PrtSc</kbd> / <kbd>Ctrl</kbd><kbd>PrtSc</kbd> | Screenshot: area / focused window / focused monitor |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>E</kbd> | Exit ymir |

See the [key-bindings][binds] wiki page for the complete list, including workspace, monitor, and column management.

## Building

First install the dependencies for your distribution:

**Ubuntu 24.04**

```sh
sudo apt-get install -y gcc clang libudev-dev libgbm-dev libxkbcommon-dev libegl1-mesa-dev libwayland-dev libinput-dev libdbus-1-dev libsystemd-dev libseat-dev libpipewire-0.3-dev libpango1.0-dev libdisplay-info-dev
```

**Fedora**

```sh
sudo dnf install gcc libudev-devel libgbm-devel libxkbcommon-devel wayland-devel libinput-devel dbus-devel systemd-devel libseat-devel pipewire-devel pango-devel cairo-gobject-devel clang libdisplay-info-devel
```

Then install [Rust](https://rustup.rs/), and build:

```sh
cargo build --release
```

Check `Cargo.toml` for the available build features. For example, to replace systemd integration with dinit integration:

```sh
cargo build --release --no-default-features --features dinit,dbus,xdp-gnome-screencast
```

> [!WARNING]
> Do **not** build with `--all-features`! Some features are only meant for development — for instance, one enables collecting profiling data into a memory buffer that grows without bound.

### Nix / NixOS

A community-maintained [flake][flake] provides a devshell with all required dependencies. Use `nix build`, then run `./result/bin/ymir`. On a non-NixOS system you may need [NixGL](https://github.com/nix-community/nixGL):

```sh
nix run --impure github:guibou/nixGL -- ./result/bin/ymir
```

### Manual installation

For a direct install without a package manager, see the [install] guide for the recommended file destinations (including the systemd and dinit unit files). The path to `ymir` in `resources/ymir.service` defaults to `/usr/bin/ymir`.

### Packaging

Community packages are available for several distributions; see [repology](https://repology.org/project/ymir/versions). The repository itself ships an [Arch PKGBUILD][PKGBUILD], a [RPM spec][rpkg], a [deb] configuration, and the [flake.nix][flake]. See [packaging][packaging] if you'd like to package ymir yourself.

## Documentation

The full documentation lives in the [wiki]. Highlights include:

- [Getting started][getting-started]
- [Configuration introduction][config-intro]
- [The Dwindle layout][dwindle]
- [Workspaces]
- [IPC]
- [Screencasting]
- [Xwayland]
- [NVIDIA][nvidia] notes
- [Security model][security]

Development documentation covers the [redraw loop][redraw], [fractional layout][fractional], [animation timing][animation], and [design principles][design].

## Status

ymir is under active development (currently `1.0.0`). It's stable enough to be a daily driver, and has a thorough test suite of unit, integration, snapshot, and property-based tests.

## Contributing

See [CONTRIBUTING.md] for guidelines. We welcome contributions — bug reports, feature requests, and pull requests alike.

## License

ymir is licensed under the [GPL-3.0-or-later][gpl] (see [LICENSE]). It is **not** intended to be a protocol-compatible Wayland implementation used by other libraries, so it uses the system wayland libraries. The [logo](#logo) is licensed under [CC BY-SA 4.0][ccbysa]; the full logo is based on the [Cherry Bomb One] font (SIL OFL 1.1).

---

[Smithay]: https://github.com/Smithay/smithay
[Alacritty]: https://github.com/alacritty/alacritty
[fuzzel]: https://codeberg.org/dnkl/fuzzel
[wiki]: https://lab.braxton.onl/braxton/ymir/wiki
[getting-started]: https://lab.braxton.onl/braxton/ymir/wiki/Getting-Started
[important software]: https://lab.braxton.onl/braxton/ymir/wiki/Important-Software
[config-intro]: https://lab.braxton.onl/braxton/ymir/wiki/Configuration:-Introduction
[binds]: https://lab.braxton.onl/braxton/ymir/wiki/Configuration:-Key-Bindings
[dwindle]: https://lab.braxton.onl/braxton/ymir/wiki/Configuration:-Dwindle
[Workspaces]: https://lab.braxton.onl/braxton/ymir/wiki/Workspaces
[IPC]: https://lab.braxton.onl/braxton/ymir/wiki/IPC
[Screencasting]: https://lab.braxton.onl/braxton/ymir/wiki/Screencasting
[Xwayland]: https://lab.braxton.onl/braxton/ymir/wiki/Xwayland
[nvidia]: https://lab.braxton.onl/braxton/ymir/wiki/Nvidia
[security]: https://lab.braxton.onl/braxton/ymir/wiki/Security-Model
[redraw]: https://lab.braxton.onl/braxton/ymir/wiki/Development:-Redraw-Loop
[fractional]: https://lab.braxton.onl/braxton/ymir/wiki/Development:-Fractional-Layout
[animation]: https://lab.braxton.onl/braxton/ymir/wiki/Development:-Animation-Timing
[design]: https://lab.braxton.onl/braxton/ymir/wiki/Development:-Design-Principles
[flake]: https://lab.braxton.onl/braxton/ymir/src/branch/main/flake.nix
[PKGBUILD]: https://lab.braxton.onl/braxton/ymir/src/branch/main/PKGBUILD
[rpkg]: https://lab.braxton.onl/braxton/ymir/src/branch/main/ymir.spec.rpkg
[deb]: https://lab.braxton.onl/braxton/ymir/src/branch/main/Cargo.toml
[packaging]: https://lab.braxton.onl/braxton/ymir/wiki/Packaging-ymir
[install]: https://lab.braxton.onl/braxton/ymir/wiki/Getting-Started#manual-installation
[CONTRIBUTING.md]: CONTRIBUTING.md
[gpl]: https://www.gnu.org/licenses/gpl-3.0.html
[LICENSE]: LICENSE
[ccbysa]: https://creativecommons.org/licenses/by-sa/4.0/
[Cherry Bomb One]: https://github.com/satsuyako/CherryBomb
