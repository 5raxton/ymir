<div align="center">

<img src="./websrc/logo.png" alt="ymir" width="220" />

# ymir

**A scrollable-tiling Wayland compositor**

</div>

ymir is a Wayland compositor written in Rust on top of [Smithay]. It arranges windows in *columns* across an infinitely-scrolling strip, and makes every new window a **Dwindle** binary-split — so tiling your screen never fights you for space.

Distributed under the [GPL-3.0-or-later](#license) license.

---

## What makes ymir different

ymir isn't just "windows scrolled side by side". Scrollable tiling is the *foundation* — the distinctive part is everything built on top of it.

### Dwindle — a real tiling tree inside every column

Where most scrollable compositors keep flat vertical stacks in each column, ymir's columns are **recursive binary-split containers** on the infinite strip. Open a new window and it splits the focused one in two, taking exactly half its space — full two-dimensional tiling that lives on a one-dimensional scrollable tape.

- Toggle split **orientation** per container, and **preselect** which side the next window takes.
- **Consume** — absorb a sibling's space into the focused window. **Expel** — pull a window out of the tree. **Promote** — move it to the head of the tree.
- **Dwindle pages** keep the tree from getting too deep: past `dwindle_windows_per_column`, new windows start a fresh full-width "page" to the right on the strip.
- Every Dwindle column still **scrolls** like any other — flip any column back to plain scrollable tiling at runtime with <kbd>Mod</kbd><kbd>Shift</kbd><kbd>D</kbd>.

### Fullscreen is just a tile

ymir treats fullscreen windows as *normal tiles on the strip* — not some special layer that swallows everything. Focus a fullscreen window and it covers the monitor; tab away and it quietly stays fullscreen, ready to come back. A concept scrollable tiling is uniquely positioned to pull off.

### Dynamic workspaces that remember where they came from

GNOME-style workspaces stacked vertically, always with one empty workspace waiting at the bottom. When you unplug a monitor, its workspaces migrate to the primary display — but each one **remembers its original output** and migrates straight back the moment that monitor reconnects. If you actively work on a workspace, it forgets its old home so your home-monitor setup doesn't get hijacked when you plug back in.

### Eye-candy that behaves

A full custom GL renderer with **gradient borders** (interpolated in CSS-equivalent color spaces — `oklch longer hue`, `oklab`, `srgb-linear`), **drop shadows** with softness/spread/offset like CSS `box-shadow`, per-window **blur**, **rounded corners**, and shader-based **open/close/resize transitions** with real spring and easing physics. VBlank-synced damage redraws keep it cheap. And when you disable an effect, it's stripped out of the render tree entirely — no wasted GPU cycles.

A focus **ring**, per-window `border`s, and an **insert hint** show exactly where a window will land while you drag.

### Gestures everywhere — not just the touchpad

Gestures work with the **mouse, touchpad, touchscreen, and tablet** alike:

- Drag with <kbd>Mod</kbd>+drag to move windows, <kbd>Mod</kbd>+right-drag to resize, <kbd>Mod</kbd>+middle-drag to scroll.
- Three-finger swipes switch workspaces and scroll the strip; **four-finger swipe** opens the Overview.
- During drag-and-drop, hold over a window to activate it, push against a screen edge to scroll, and drop onto another workspace in the Overview.
- The **top-left hot corner** flips into the Overview.

### The Overview

A zoomed-out look at all your workspaces at once. Drag windows between them, grab, drop, and organize purely with the mouse — no keyboard required.

### Thoughtful screencasting

- **Block out** sensitive windows (password managers, messengers) as solid black rects in casts via a window rule.
- **Dynamic cast target** — one stream that follows whatever window or monitor you focus, on demand.
- **Windowed fullscreen** — tell Google Slides it's fullscreen while leaving it as a normal, resizable window on your ultrawide.
- Get a red border on every window that's currently being screencasted.

### Finished-in, not bolted-on

- **Lua configuration** (`~/.config/ymir/init.lua`) — window rules and layer rules with regex matching, imperative `ymir.*` prelude, includes, and live reload.
- **Built-in screenshot UI** — an area picker that saves straight to disk, no separate tool.
- **Recent-windows switcher** (Alt-Tab) with live previews, scope filtering (all / output / workspace / app), and binds that adapt to your layout.
- **IPC** — a Unix-socket JSON API plus a `ymir msg` CLI for remote control.
- Full **accessibility** via accesskit + AT-SPI, **multi-GPU** support, **Xwayland** via Xwayland-satellite, and systemd & dinit integration.

---

## Features at a glance

| Area | What ymir does |
| --- | --- |
| Layout | Scrollable tiling, Dwindle binary-split trees, floating windows, dwindle pages, per-workspace & per-output layout overrides |
| Windows | Fullscreen as a tile, maximize, windowed fullscreen, Overview, recent-windows switcher, window rules, focus ring, borders, insert hints, gaps, struts |
| Effects | Gradient borders (multiple color spaces), blur, drop shadows, rounded corners, opacity, spring/easing animations |
| Input | Mouse, touchpad, touchscreen, tablet & trackpoint via libinput; grabs, gestures, hot corner, shortcut inhibit, power-off monitors |
| Gestures | `<Mod>`-drag move/resize, 2/3/4-finger touchpad, touch+tablet move, DnD edge scroll, hold-to-activate, overview gestures |
| Desktop | Systemd & dinit, D-Bus (freedesktop + GNOME/Mutter interfaces), portals, screen locking, system tray |
| Accessibility | Full screen-reader support via accesskit + AT-SPI |
| Multi-GPU | Render-device detection, dmabuf feedback, texture copy between GPUs |
| IPC | Unix-socket JSON API + `ymir msg` CLI |
| Screencasting | Output/window casts via portal+PipeWire, block-out, dynamic cast target, mirroring |
| Xwayland | Via Xwayland-satellite, on demand |

## Getting started

The quickest path is the multi-distro installer (detects Arch, Fedora, Debian/Ubuntu, openSUSE). It supports two modes — **binary** (default) and **source**:

```sh
# bash/zsh — binary mode (download the newest pre-built binary for your distro)
curl -sL https://lab.braxton.onl/braxton/ymir/raw/branch/main/scripts/install.sh | bash

# fish — binary mode
curl -sL https://lab.braxton.onl/braxton/ymir/raw/branch/main/scripts/install.fish | fish

# build from source (newest main) instead of downloading a binary
curl -sL https://lab.braxton.onl/braxton/ymir/raw/branch/main/scripts/install.sh | bash -s -- --source
```

You can also select the mode with `YMIR_MODE=binary|source`, or run a locally-checked-out copy directly (`bash scripts/install.sh --source`).

The installer seeds `~/.config/ymir/init.lua` with the default Dwindle config if absent and installs the `ymir.desktop` session entry so **Ymir** appears in your display manager (GDM, SDDM, …). From a bare TTY, start with `ymir-session`. Re-running it reinstalls the newest ymir — in binary mode it always pulls the newest pre-built release, and in source mode it clones the newest `main` and rebuilds.

Inside a session:

- <kbd>Super</kbd><kbd>T</kbd> opens a terminal ([Alacritty])
- <kbd>Super</kbd><kbd>D</kbd> opens an app launcher ([fuzzel])
- <kbd>Super</kbd><kbd>Shift</kbd><kbd>E</kbd> exits ymir

The default config runs Waybar and assumes a portal/screencast stack; see the [wiki] for the full list of [important software] and how to [configure][config-intro] ymir.

> **Note:** `ymir` can also be opened *as a window* from inside an existing desktop session for a quick try. This windowed mode is mainly for development and can be a little buggy (especially hotkeys).

### Main default hotkeys

When running on a TTY the mod key is <kbd>Super</kbd>; in windowed dev mode it's <kbd>Alt</kbd>. As a rule of thumb, adding <kbd>Ctrl</kbd> to a *switch* hotkey *moves* the focused window/column instead; <kbd>Shift</kbd> does an *alternative* action (e.g. cross-monitor movement, or window height instead of width).

| Hotkey | Description |
| --- | --- |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>/</kbd> | Show a list of important ymir hotkeys |
| <kbd>Mod</kbd><kbd>T</kbd> / <kbd>Mod</kbd><kbd>D</kbd> | Spawn `alacritty` / `fuzzel` |
| <kbd>Mod</kbd><kbd>Q</kbd> | Close the focused window |
| <kbd>Mod</kbd><kbd>H</kbd><kbd>L</kbd> or <kbd>←</kbd><kbd>→</kbd> | Focus the column left / right |
| <kbd>Mod</kbd><kbd>J</kbd><kbd>K</kbd> or <kbd>↓</kbd><kbd>↑</kbd> | Focus the window below / above in a column |
| <kbd>Mod</kbd><kbd>U</kbd> / <kbd>Mod</kbd><kbd>I</kbd> | Switch to the workspace below / above |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>D</kbd> | Switch the focused column between Dwindle and scrollable tiling |
| <kbd>Mod</kbd><kbd>Space</kbd> | Toggle the split orientation (Dwindle) |
| <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>Space</kbd> | Preselect the next split direction (Dwindle) |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>Home</kbd> | Move the focused window to the head of its Dwindle tree |
| <kbd>Mod</kbd><kbd>[</kbd> / <kbd>Mod</kbd><kbd>]</kbd> | Consume / expel the focused window |
| <kbd>Mod</kbd><kbd>R</kbd> / <kbd>Mod</kbd><kbd>Shift</kbd><kbd>R</kbd> | Cycle preset column widths forward / back |
| <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>R</kbd> | Cycle preset window heights |
| <kbd>Mod</kbd><kbd>M</kbd> | Maximize window |
| <kbd>Mod</kbd><kbd>V</kbd> / <kbd>Mod</kbd><kbd>Shift</kbd><kbd>V</kbd> | Move / switch focus between floating and tiling |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>F</kbd> | Toggle fullscreen |
| <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>F</kbd> | Toggle windowed fullscreen |
| <kbd>Mod</kbd><kbd>-</kbd> / <kbd>Mod</kbd><kbd>=</kbd> | Decrease / increase column width by 10% |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>-</kbd> / <kbd>Mod</kbd><kbd>Shift</kbd><kbd>=</kbd> | Decrease / increase window height by 10% |
| <kbd>Alt</kbd><kbd>Tab</kbd> | Recent-windows switcher with previews |
| <kbd>PrtSc</kbd> / <kbd>Alt</kbd><kbd>PrtSc</kbd> / <kbd>Ctrl</kbd><kbd>PrtSc</kbd> | Screenshot: area / focused window / focused monitor |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>E</kbd> | Exit ymir |

See the [key-bindings][binds] wiki page for the complete list, including workspace, monitor, and Dwindle management.

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

Then install [Rust](https://rustup.rs/) and build:

```sh
cargo build --release
```

Check `Cargo.toml` for the available build features. For example, to replace systemd integration with dinit integration:

```sh
cargo build --release --no-default-features --features dinit,dbus,xdp-gnome-screencast
```

> [!WARNING]
> Do **not** build with `--all-features`! Some features are only meant for development — one of them collects profiling data into a memory buffer that grows without bound.

### Nix / NixOS

A community-maintained [flake][flake] provides a devshell with all required dependencies. Use `nix build`, then run `./result/bin/ymir`. On a non-NixOS system you may need [NixGL](https://github.com/nix-community/nixGL):

```sh
nix run --impure github:guibou/nixGL -- ./result/bin/ymir
```

### Manual installation

For a direct install without a package manager, see the [install] guide for recommended file destinations (including the systemd and dinit units). The path to `ymir` in `resources/ymir.service` defaults to `/usr/bin/ymir`.

### Packaging

Community packages exist for several distributions; see [repology](https://repology.org/project/ymir/versions). The repo also ships an [Arch PKGBUILD][PKGBUILD], an [RPM spec][rpkg], a [deb] configuration, and a [flake.nix][flake]. See [packaging][packaging] if you'd like to package ymir yourself.

## Design principles

ymir is guided by a handful of explicit [design principles][design], worth knowing if you want to understand the project:

1. **Opening a new window should not affect the sizes of any existing windows.**
2. **The focused window should not move around on its own.**
3. **Actions should apply immediately** — resize and switch instantly, even while an animation is still playing.
4. **When disabled, eye-candy features don't affect performance.**
5. **Eye-candy shouldn't cause unreasonable excessive rendering** — e.g. rounded corners still allow direct scanout when possible.
6. **Be mindful of invisible state** — the "original output" tracking is the canonical example.

## Documentation

The full documentation lives in the [wiki]. Highlights include:

- [Getting started][getting-started]
- [Configuration introduction][config-intro]
- [The Dwindle layout][dwindle]
- [The Overview][overview]
- [Gestures]
- [Workspaces]
- [Layout configuration][layout]
- [Recent windows][recent]
- [Screencasting]
- [IPC]
- [Xwayland]
- [NVIDIA][nvidia] notes
- [Security model][security]

Development docs cover the [redraw loop][redraw], [fractional layout][fractional], [animation timing][animation], and [design principles][design].

## Status

Under active development (currently `1.0.0`) and stable enough to be a daily driver, backed by a thorough suite of unit, integration, snapshot, and property-based tests. See [CONTRIBUTING.md] to get involved.

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
[overview]: https://lab.braxton.onl/braxton/ymir/wiki/Overview
[Gestures]: https://lab.braxton.onl/braxton/ymir/wiki/Gestures
[Workspaces]: https://lab.braxton.onl/braxton/ymir/wiki/Workspaces
[layout]: https://lab.braxton.onl/braxton/ymir/wiki/Configuration:-Layout
[recent]: https://lab.braxton.onl/braxton/ymir/wiki/Configuration:-Recent-Windows
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
