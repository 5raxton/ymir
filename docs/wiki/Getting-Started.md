## Quick start

ymir ships a multi-distro installer script that detects your distro (Arch, Fedora, Debian/Ubuntu, openSUSE), installs the required build/runtime dependencies, clones the latest `main`, and builds & installs the compositor:

```sh
# bash/zsh
curl -sL https://lab.braxton.onl/braxton/ymir/raw/branch/main/scripts/install.sh | bash

# fish
curl -sL https://lab.braxton.onl/braxton/ymir/raw/branch/main/scripts/install.fish | fish
```

On Arch it builds with `makepkg` from the PKGBUILD; on other distros it builds with `cargo` and installs into `/usr/local`. The installer seeds `~/.config/ymir/init.lua` with the default dwindle config if absent, and installs the `ymir.desktop` session entry automatically so "Ymir" appears in your login manager (GDM, SDDM, ...). From a bare TTY you can start it directly with `ymir-session`. Re-running the installer pulls the latest `main` and rebuilds, so it also doubles as a bleeding-edge update.

Alternatively, some distributions provide packaged builds of ymir — see the ["Slower and more considered start"](#slower-and-more-considered-start) section. You can also try a more out-of-the-box experience with [DankMaterialShell](https://github.com/AvengeMedia/DankMaterialShell):

Fedora:
```
sudo dnf copr enable avengemedia/dms
sudo dnf install ymir dms
systemctl --user add-wants ymir.service dms
```

Arch Linux:
```
sudo pacman -Syu ymir xwayland-satellite xdg-desktop-portal-gnome xdg-desktop-portal-gtk alacritty dms-shell-ymir matugen cava qt6-multimedia-ffmpeg
systemctl --user add-wants ymir.service dms
```

Ubuntu 25.10 and above:
```
sudo add-apt-repository ppa:avengemedia/danklinux
sudo add-apt-repository ppa:avengemedia/dms
sudo apt install ymir dms
```

After running these commands, log out, choose Ymir in your display manager, and log back in.
Or, if not using a display manager, run `ymir-session` on a TTY.

The default ymir config will run Waybar, so you might get two bars on screen.
To fix this, stop Waybar with `pkill waybar` command, then open `~/.config/ymir/init.lua` and delete the `spawn_at_startup = { { command = { "waybar" } } }` line.

Check the DankMaterialShell's [compositor setup page](https://danklinux.com/docs/dankmaterialshell/compositors#ymir-configuration) to learn how to configure DMS-specific binds and other ymir integrations.

## Slower and more considered start

Aside from the official installer above, ymir is available as a number of distribution packages maintained by the community.
Here are some of them: [Fedora COPR](https://copr.fedorainfracloud.org/coprs/yalter/ymir/) and [nightly COPR](https://copr.fedorainfracloud.org/coprs/yalter/ymir-git/), [NixOS Flake](https://github.com/epireyn/ymir-flake) (maintained fork of [sodiboo/ymir-flake](https://github.com/sodiboo/ymir-flake)), and some more from repology below, including a [pacstall package](https://pacstall.dev/packages/ymir/) for Debian-based distros.
This repository also ships its own [flake.nix](https://lab.braxton.onl/braxton/ymir/src/branch/main/flake.nix) and an [Arch PKGBUILD](https://lab.braxton.onl/braxton/ymir/src/branch/main/PKGBUILD).
See the [Building](#building) section if you'd like to compile ymir yourself and the [Packaging ymir](./Packaging-ymir.md) page if you want to package ymir.

[![Packaging status](https://repology.org/badge/vertical-allrepos/ymir.svg)](https://repology.org/project/ymir/versions)

After installing, start ymir from your display manager like GDM.
Press <kbd>Super</kbd><kbd>T</kbd> to run a terminal ([Alacritty]) and <kbd>Super</kbd><kbd>D</kbd> to run an application launcher ([fuzzel]).
To exit ymir, press <kbd>Super</kbd><kbd>Shift</kbd><kbd>E</kbd>.

If you're not using a display manager, you should run `ymir-session` (systemd/dinit) or `ymir --session` (others) from a TTY.
The `--session` flag will make ymir import its environment variables globally into the system manager and D-Bus, and start its D-Bus services.
The `ymir-session` script will additionally start ymir as a systemd/dinit service, which starts up a graphical session target required by some services like portals.

You can also run `ymir` inside an existing desktop session.
Then it will open as a window, where you can give it a try.
Note that this windowed mode is mainly meant for development, so it is a bit buggy (in particular, there are issues with hotkeys).

Next, see the [list of important software](./Important-Software.md) required for normal desktop use, like a notification daemon and portals.
Also, check the [configuration introduction](./Configuration:-Introduction.md) page to get started configuring ymir.
There you can find links to other pages containing thorough documentation and examples for all options.
Finally, the [Xwayland](./Xwayland.md) page explains how to run X11 applications on ymir.

### Desktop environments

Some desktop environments and shells work with ymir and can give a more out-of-the-box experience:

- [LXQt](https://lxqt-project.org/) officially supports ymir, see [their wiki](https://github.com/lxqt/lxqt/wiki/ConfigWaylandSettings#general) for details on setting it up.
- Many [XFCE](https://www.xfce.org/) components work on Wayland, including ymir. See [their wiki](https://wiki.xfce.org/releng/wayland_roadmap#component_specific_status) for details.
- There are complete desktop shells based on Quickshell that support ymir, for example [DankMaterialShell](https://github.com/AvengeMedia/DankMaterialShell) and [Noctalia](https://github.com/noctalia-dev/noctalia-shell).
- You can run a [COSMIC](https://system76.com/cosmic/) session with ymir using [cosmic-ext-extra-sessions](https://github.com/Drakulix/cosmic-ext-extra-sessions).

### NVIDIA

The NVIDIA drivers currently have an issue with high VRAM usage due to a heap reuse quirk.
You're recommended to apply a manual fix documented [here](./Nvidia.md) if you run ymir on an NVIDIA GPU.

NVIDIA GPUs can have problems running ymir (for example, the screen remains black upon starting from a TTY).
Sometimes, the problems can be fixed.
You can try the following:

1. Update NVIDIA drivers. You need a GPU and drivers recent enough to support GBM.
2. Make sure kernel modesetting is enabled. This usually involves adding `nvidia-drm.modeset=1` to the kernel command line. Find and follow a guide for your distribution. Guides from other Wayland compositors can help.

### Asahi, ARM, and other kmsro devices

On some of these systems, ymir fails to correctly detect the primary render device.
If you're getting a black screen when starting ymir on a TTY, you can try to set the device manually.

First, find which devices you have:

```
$ ls -l /dev/dri/
drwxr-xr-x@       - root 14 мая 07:07 by-path
crw-rw----@   226,0 root 14 мая 07:07 card0
crw-rw----@   226,1 root 14 мая 07:07 card1
crw-rw-rw-@ 226,128 root 14 мая 07:07 renderD128
crw-rw-rw-@ 226,129 root 14 мая 07:07 renderD129
```

You will likely have one `render` device and two `card` devices.

Open the ymir config file at `~/.config/ymir/init.lua` and put your `render` device path like this:

```lua
return {
    debug = {
        render_drm_device = "/dev/dri/renderD128",
    },
}
```

Save, then try to start ymir again.
If you still get a black screen, try using each of the `card` devices.

### Nix/NixOS

There's a common problem of mesa drivers going out of sync with ymir, so make sure your system mesa version matches the ymir mesa version.
When this happens, you usually see a black screen when trying to start ymir from a TTY.

Also, on Intel graphics, you may need a workaround described [here](https://wiki.nixos.org/wiki/Intel_Graphics).

### Virtual Machines

To run ymir in a VM, make sure to enable 3D acceleration.

## Main Default Hotkeys

When running on a TTY, the Mod key is <kbd>Super</kbd>.
When running in a window, the Mod key is <kbd>Alt</kbd>.

The general system is: if a hotkey switches somewhere, then adding <kbd>Ctrl</kbd> will move the focused window or column there.

The default column layout is [Dwindle](./Configuration:-Dwindle.md): new windows split off the focused window into a binary-split tree. You can switch a column back to classic scrollable tiling at any time with <kbd>Mod</kbd><kbd>Shift</kbd><kbd>D</kbd>.

| Hotkey | Description |
| ------ | ----------- |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>/</kbd> | Show a list of important ymir hotkeys |
| <kbd>Mod</kbd><kbd>T</kbd> | Spawn `alacritty` (terminal) |
| <kbd>Mod</kbd><kbd>D</kbd> | Spawn `fuzzel` (application launcher) |
| <kbd>Super</kbd><kbd>Alt</kbd><kbd>L</kbd> | Spawn `swaylock` (screen locker) |
| <kbd>Mod</kbd><kbd>Q</kbd> | Close the focused window |
| <kbd>Mod</kbd><kbd>H</kbd> or <kbd>Mod</kbd><kbd>←</kbd> | Focus the column to the left |
| <kbd>Mod</kbd><kbd>L</kbd> or <kbd>Mod</kbd><kbd>→</kbd> | Focus the column to the right |
| <kbd>Mod</kbd><kbd>J</kbd> or <kbd>Mod</kbd><kbd>↓</kbd> | Focus the window below in a column |
| <kbd>Mod</kbd><kbd>K</kbd> or <kbd>Mod</kbd><kbd>↑</kbd> | Focus the window above in a column |
| <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>H</kbd> or <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>←</kbd> | Move the focused column to the left |
| <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>L</kbd> or <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>→</kbd> | Move the focused column to the right |
| <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>J</kbd> or <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>↓</kbd> | Move the focused window below in a column |
| <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>K</kbd> or <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>↑</kbd> | Move the focused window above in a column |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>Down</kbd> or <kbd>Mod</kbd><kbd>Shift</kbd><kbd>J</kbd> | Focus the monitor below |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>Up</kbd> or <kbd>Mod</kbd><kbd>Shift</kbd><kbd>K</kbd> | Focus the monitor above |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>←</kbd><kbd>→</kbd> or <kbd>Mod</kbd><kbd>Shift</kbd><kbd>H</kbd><kbd>L</kbd> | Move the focused window spatially in the dwindle tree |
| <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>H</kbd><kbd>J</kbd><kbd>K</kbd><kbd>L</kbd> or <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>←</kbd><kbd>↓</kbd><kbd>↑</kbd><kbd>→</kbd> | Move the focused column to the monitor to the side |
| <kbd>Mod</kbd><kbd>U</kbd> or <kbd>Mod</kbd><kbd>PageDown</kbd> | Switch to the workspace below |
| <kbd>Mod</kbd><kbd>I</kbd> or <kbd>Mod</kbd><kbd>PageUp</kbd> | Switch to the workspace above |
| <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>U</kbd> or <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>PageDown</kbd> | Move the focused column to the workspace below |
| <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>I</kbd> or <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>PageUp</kbd> | Move the focused column to the workspace above |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>U</kbd> or <kbd>Mod</kbd><kbd>Shift</kbd><kbd>PageDown</kbd> | Move the focused workspace down |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>I</kbd> or <kbd>Mod</kbd><kbd>Shift</kbd><kbd>PageUp</kbd> | Move the focused workspace up |
| <kbd>Mod</kbd><kbd>[</kbd> | Consume or expel the focused window to the left |
| <kbd>Mod</kbd><kbd>]</kbd> | Consume or expel the focused window to the right |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>D</kbd> | Switch the focused column between dwindle and scrollable tiling |
| <kbd>Mod</kbd><kbd>Space</kbd> | Toggle the split orientation of the container holding the focused window (dwindle) |
| <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>Space</kbd> | Preselect the split direction for the next window in the focused column (dwindle) |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>Home</kbd> | Move the focused window to the head of its dwindle tree |
| <kbd>Mod</kbd><kbd>R</kbd> and <kbd>Mod</kbd><kbd>Shift</kbd><kbd>R</kbd> | Toggle between preset column widths forward and back |
| <kbd>Mod</kbd><kbd>M</kbd> | Maximize window |
| <kbd>Mod</kbd><kbd>C</kbd> | Center column within view |
| <kbd>Mod</kbd><kbd>-</kbd> | Decrease column width by 10% |
| <kbd>Mod</kbd><kbd>=</kbd> | Increase column width by 10% |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>-</kbd> | Decrease window height by 10% |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>=</kbd> | Increase window height by 10% |
| <kbd>Mod</kbd><kbd>Ctrl</kbd><kbd>R</kbd> | Reset window height back to automatic |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>F</kbd> | Toggle full-screen on the focused window |
| <kbd>Mod</kbd><kbd>V</kbd> | Move the focused window between the floating and the tiling layout |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>V</kbd> | Switch focus between the floating and the tiling layout |
| <kbd>PrtSc</kbd> | Take an area screenshot. Select the area to screenshot with mouse, then press Space to save the screenshot, or Escape to cancel |
| <kbd>Alt</kbd><kbd>PrtSc</kbd> | Take a screenshot of the focused window to clipboard and to `~/Pictures/Screenshots/` |
| <kbd>Ctrl</kbd><kbd>PrtSc</kbd> | Take a screenshot of the focused monitor to clipboard and to `~/Pictures/Screenshots/` |
| <kbd>Mod</kbd><kbd>Shift</kbd><kbd>E</kbd> or <kbd>Ctrl</kbd><kbd>Alt</kbd><kbd>Delete</kbd> | Exit ymir |

## Building

First, install the dependencies for your distribution.

- Ubuntu 24.04:

    ```sh
    sudo apt-get install -y gcc clang libudev-dev libgbm-dev libxkbcommon-dev libegl1-mesa-dev libwayland-dev libinput-dev libdbus-1-dev libsystemd-dev libseat-dev libpipewire-0.3-dev libpango1.0-dev libdisplay-info-dev
    ```

- Fedora:

    ```sh
    sudo dnf install gcc libudev-devel libgbm-devel libxkbcommon-devel wayland-devel libinput-devel dbus-devel systemd-devel libseat-devel pipewire-devel pango-devel cairo-gobject-devel clang libdisplay-info-devel
    ```

Next, get latest stable Rust: https://rustup.rs/

Then, build ymir with `cargo build --release`.

Check Cargo.toml for a list of build features.
For example, you can replace systemd integration with dinit integration using `cargo build --release --no-default-features --features dinit,dbus,xdp-gnome-screencast`.

> [!WARNING]
> Do NOT build with `--all-features`!
>
> Some features are meant only for development use.
> For example, one of the features enables collection of profiling data into a memory buffer that will grow indefinitely until you run out of memory.

### NixOS/Nix

We have a community-maintained flake which provides a devshell with required dependencies. Use `nix build` to build ymir, and then run `./results/bin/ymir`.

If you're not on NixOS, you may need [NixGL](https://github.com/nix-community/nixGL) to run the resulting binary:

```sh
nix run --impure github:guibou/nixGL -- ./results/bin/ymir
```

### Manual Installation

If installing directly without a package, the recommended file destinations are slightly different.
In this case, put the files in the directories indicated in the table below.
These may vary depending on your distribution.

Don't forget to make sure that the path to `ymir` in ymir.service is correct.
This defaults to `/usr/bin/ymir`.

| File | Destination |
| ---- | ----------- |
| `target/release/ymir` | `/usr/local/bin/` |
| `resources/ymir-session` | `/usr/local/bin/` |
| `resources/ymir.desktop`  | `/usr/local/share/wayland-sessions/` |
| `resources/ymir-portals.conf` | `/usr/local/share/xdg-desktop-portal/` |
| `resources/ymir.service` (systemd) | `/etc/systemd/user/` |
| `resources/ymir-shutdown.target` (systemd) | `/etc/systemd/user/` |
| `resources/dinit/ymir` (dinit) | `/etc/dinit.d/user/` |
| `resources/dinit/ymir.target` (dinit) | `/etc/dinit.d/user/` |

[Alacritty]: https://github.com/alacritty/alacritty
[fuzzel]: https://codeberg.org/dnkl/fuzzel
