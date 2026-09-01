<div align="center">

<img src="./websrc/logo.png" alt="ymir logo" width="220" />

# ymir

**Infinite scroll meets binary-split tiling.**

</div>

**ymir** is a Wayland compositor written in Rust on top of [Smithay]. It rethinks screen real estate by arranging windows across an infinitely-scrolling strip, while making every new window a **Dwindle** binary-split. The result? A tiling experience that never fights you for space.

Distributed under the [GPL-3.0-or-later](#license) license.

---

## ⚡ The ymir Philosophy

ymir isn't just another compositor putting windows side-by-side. Scrollable tiling is the foundation, but the true power lies in what's built on top of it.

### Dwindle: Two-Dimensional Tiling on a One-Dimensional Strip
Where most scrollable compositors rely on flat vertical stacks, ymir's columns are **recursive binary-split containers**. Open a new window, and it splits the focused one in half. You get the power of a full 2D tiling tree that lives on an infinitely scrollable 1D tape.

* **Limitless Expansion:** Customize the `dwindle_windows_per_column` limit. Once hit, ymir automatically wraps new windows into a fresh, full-width "page" to the right—no manual workspace switching required.
* **Total Control:** Toggle split orientation per container, preselect the next split direction, or **consume** a sibling's space entirely. 
* **Seamless Fallback:** Need a traditional layout? Hit <kbd>Mod</kbd><kbd>Shift</kbd><kbd>D</kbd> to instantly revert any Dwindle column back to standard scrollable tiling.

### Fullscreen is Just a Tile
Fullscreen doesn't hijack your entire workflow—it's just a tile on the strip. Focus it, and it covers the monitor; tab away, and it quietly remains fullscreen in its column, waiting for you to scroll back. 

### Workspaces with Memory
Vertical, GNOME-style workspaces that actually respect your hardware. Unplug a monitor, and its workspaces seamlessly migrate to your primary display. Plug it back in, and they **migrate straight back**. Workspaces only "forget" their original home if you actively start working on them while migrated, keeping your multi-monitor setup intact.

### Uncompromising, Disciplined Rendering
A fully custom GL renderer built for speed and aesthetics, utilizing vblank-synced damage redraws to keep GPU overhead minimal.
* **Premium Eye-Candy:** Gradient borders (interpolated in accurate color spaces like `oklch`, `oklab`, `srgb-linear`), CSS-style drop shadows, per-window blur, and rounded corners that still allow for direct scanout where possible.
* **Physics-Based Animation:** Shader-driven open/close/resize transitions with real spring and easing physics.
* **Zero-Waste:** Disable an effect, and it gets stripped from the render tree entirely. No wasted GPU cycles.

### Intuitive, Omnipresent Gestures
Navigating ymir feels fluid whether you are on a mouse, touchpad, touchscreen, or tablet:
* <kbd>Mod</kbd>+Drag to move, right-drag to resize, middle-drag to scroll.
* 3-finger swipes for workspaces and scrolling; 4-finger swipes for the Overview.
* Top-left hot corner and hold-to-activate drag-and-drop mechanics.

### Thoughtful Screencasting
* **Privacy First:** Block sensitive windows (like password managers) as solid black rectangles in your casts using simple window rules.
* **Dynamic Targets:** Broadcast a single stream that automatically follows your focused window or monitor.
* **Windowed Fullscreen:** Tell apps like Google Slides they are fullscreen, while keeping them as perfectly normal, resizable tiles on your ultrawide monitor.

### Built-In, Not Bolted-On
* **Lua Configuration:** Live-reloading `~/.config/ymir/init.lua` with regex matching and an imperative `ymir.*` API.
* **Integrated Screenshot UI:** Area picker that saves straight to disk—no third-party bloat required.
* **Smart Window Switcher:** <kbd>Alt</kbd><kbd>Tab</kbd> with live previews and scope filtering (all / output / workspace / app).
* **Robust IPC:** Unix-socket JSON API and a `ymir msg` CLI for complete remote control.

---

## 🛠️ Features at a Glance

| Category | Capabilities |
| :--- | :--- |
| **Layout** | Scrollable tiling, Dwindle binary-split, floating mode, dwindle pages, per-output/workspace overrides. |
| **Window Ops** | Fullscreen-as-tile, windowed fullscreen, Overview mode, live-preview switcher, focus rings, insert hints, struts. |
| **Visuals** | Multi-color-space gradient borders, blur, drop shadows, rounded corners, spring physics animations. |
| **Input** | Libinput support (mouse, touchpad, touch, tablet, trackpoint), hot corners, shortcut inhibits. |
| **Gestures** | 2/3/4-finger touchpad swipes, edge-scrolling, touch/tablet drag-and-drop integration. |
| **System** | Systemd/dinit integration, D-Bus (freedesktop/Mutter), portals, screen locking, system tray. |
| **Hardware** | Multi-GPU render device detection, dmabuf feedback, texture copying across GPUs. |
| **Compat** | Full screen-reader support via accesskit + AT-SPI, Xwayland (via satellite), portal+PipeWire casting. |

---

## 🚀 Getting Started

The fastest way to get ymir running is our multi-distro installer (supports Arch, Fedora, Debian/Ubuntu, openSUSE). 

```sh
# Binary mode (Downloads the latest pre-built release)
curl -sL https://lab.braxton.onl/braxton/ymir/raw/branch/main/scripts/install.sh | bash

# Source mode (Clones main and builds locally)
curl -sL https://lab.braxton.onl/braxton/ymir/raw/branch/main/scripts/install.sh | bash -s -- --source
```
*Note: Fish users can swap `install.sh | bash` with `install.fish | fish`.*

The installer seeds your `~/.config/ymir/init.lua`, sets up the default Dwindle config, and installs the `ymir.desktop` session for your display manager. From a bare TTY, simply run `ymir-session`.

> **Dev Mode:** You can open ymir *as a window* inside your existing desktop session to test it out (note: some hotkeys may conflict with your host compositor).

### Essential Hotkeys
*(Mod is <kbd>Super</kbd> in a TTY session, <kbd>Alt</kbd> in windowed dev mode)*

| Action | Hotkey |
| :--- | :--- |
| **Terminal / Launcher** | <kbd>Mod</kbd><kbd>T</kbd> / <kbd>Mod</kbd><kbd>D</kbd> |
| **Close Window** | <kbd>Mod</kbd><kbd>Q</kbd> |
| **Navigate Columns** | <kbd>Mod</kbd><kbd>H</kbd><kbd>L</kbd> or <kbd>←</kbd><kbd>→</kbd> |
| **Navigate Inside Column**| <kbd>Mod</kbd><kbd>J</kbd><kbd>K</kbd> or <kbd>↓</kbd><kbd>↑</kbd> |
| **Toggle Dwindle/Scroll** | <kbd>Mod</kbd><kbd>Shift</kbd><kbd>D</kbd> |
| **Toggle Split Axis** | <kbd>Mod</kbd><kbd>Space</kbd> |
| **Consume / Expel** | <kbd>Mod</kbd><kbd>[</kbd> / <kbd>Mod</kbd><kbd>]</kbd> |
| **Toggle Fullscreen** | <kbd>Mod</kbd><kbd>Shift</kbd><kbd>F</kbd> |
| **Screenshot Menu** | <kbd>PrtSc</kbd> |
| **Exit ymir** | <kbd>Mod</kbd><kbd>Shift</kbd><kbd>E</kbd> |

Press <kbd>Mod</kbd><kbd>Shift</kbd><kbd>/</kbd> to view the full cheat sheet, or check the [Wiki Key-Bindings][binds].

---

## 🏗️ Building from Source

Ensure you have [Rust](https://rustup.rs/) installed, along with your distribution's development libraries.

<details>
<summary><b>Ubuntu 24.04 Dependencies</b></summary>

```sh
sudo apt-get install -y gcc clang libudev-dev libgbm-dev libxkbcommon-dev libegl1-mesa-dev libwayland-dev libinput-dev libdbus-1-dev libsystemd-dev libseat-dev libpipewire-0.3-dev libpango1.0-dev libdisplay-info-dev
```
</details>

<details>
<summary><b>Fedora Dependencies</b></summary>

```sh
sudo dnf install gcc libudev-devel libgbm-devel libxkbcommon-devel wayland-devel libinput-devel dbus-devel systemd-devel libseat-devel pipewire-devel pango-devel cairo-gobject-devel clang libdisplay-info-devel
```
</details>

**Build Command:**
```sh
cargo build --release
```
*Note: Check `Cargo.toml` for feature flags. Avoid `--all-features` as it enables dev-only memory-heavy profiling.*

### NixOS
We maintain a [flake][flake] providing a complete devshell.
```sh
nix build
./result/bin/ymir
```
*(Non-NixOS systems may require [NixGL](https://github.com/nix-community/nixGL)).*

---

## 📐 Design Principles

ymir is built on strict architectural rules to ensure the compositor never gets in your way:

1. **Space is Sacred:** Opening a new window will *never* randomly resize your existing, perfectly-placed windows.
2. **Deterministic Focus:** The focused window does not move on its own.
3. **Zero Latency:** Actions apply immediately. You can resize and switch workspaces instantly, even mid-animation.
4. **Lean Rendering:** Disabled eye-candy costs zero performance. 
5. **Smart Scanout:** Eye-candy shouldn't force excessive rendering (e.g., rounded corners map to direct scanout when possible).
6. **State Awareness:** Invisible state (like tracking a workspace's "original output") is handled meticulously.

---

## 📚 Documentation & Status

ymir is actively developed (currently `1.0.0`) and rigorously tested via unit, integration, snapshot, and property-based tests. It is stable enough to be your daily driver.

Dive into the [Wiki][wiki] for deep-dives:
* [Configuration (Lua)][config-intro]
* [Mastering the Dwindle Layout][dwindle]
* [Screencasting Setup][Screencasting]
* [Xwayland & NVIDIA Notes][nvidia]
* [Development & Render Loops][redraw]

Want to contribute? Check out [CONTRIBUTING.md]. 

**License:** [GPL-3.0-or-later][gpl].
