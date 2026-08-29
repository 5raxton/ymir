#!/usr/bin/env fish
# ymir Arch Linux installer (fish version).
#
#   - installs the build/runtime dependencies from pacman
#   - clones (or updates) the repo from origin main
#   - builds and installs ymir via makepkg -si
#   - seeds ~/.config/ymir/config.kdl with the dwindle example config if absent
#
# Safe to re-run any time to pull updates from main and reinstall.

if test (id -u) -eq 0
    echo "error: do not run this script as root; makepkg must run as a regular user." >&2
    exit 1
end

if not command -q pacman
    echo "error: this installer targets Arch Linux (pacman was not found)." >&2
    exit 1
end

set -q YMIR_REPO_DIR; or set YMIR_REPO_DIR "$HOME/src/ymir"
set -q YMIR_BRANCH;   or set YMIR_BRANCH main
set REPO_URL "git@github.com:5raxton/ymir.git"

set DEPS \
    base-devel \
    cairo \
    fontconfig \
    freetype2 \
    git \
    harfbuzz \
    libdrm \
    libdisplay-info \
    libglvnd \
    libinput \
    libxcb \
    libxkbcommon \
    mesa \
    pango \
    pkgconf \
    pipewire \
    rust \
    seatd \
    wayland \
    wayland-protocols

echo ">>> Installing build/runtime dependencies"
sudo pacman -S --needed --noconfirm $DEPS
or exit 1

mkdir -p (dirname "$YMIR_REPO_DIR")
if test -d "$YMIR_REPO_DIR/.git"
    echo ">>> Updating ymir in $YMIR_REPO_DIR"
    # makepkg rewrites the PKGBUILD's pkgver line when building, so throw any
    # such local edits away before pulling.
    git -C "$YMIR_REPO_DIR" fetch origin "$YMIR_BRANCH"
    or exit 1
    git -C "$YMIR_REPO_DIR" reset --hard "origin/$YMIR_BRANCH"
    or exit 1
else
    echo ">>> Cloning ymir into $YMIR_REPO_DIR"
    git clone --branch "$YMIR_BRANCH" "$REPO_URL" "$YMIR_REPO_DIR"
    or exit 1
end

echo ">>> Building and installing ymir (release build, this takes a while)"
set OLD_DIR (pwd)
cd "$YMIR_REPO_DIR"
# Force a clean build (-C): the Cargo target/ dir left over inside the
# makepkg srcdir by previous runs can poison the final link (stale libspa-sys
# artifacts / missing libspa_rs symbols). A from-scratch build always links.
makepkg -sCci --noconfirm --needed
or exit 1
cd "$OLD_DIR"

# The session .desktop file is what makes "Ymir" show up in the greeter
# (GDM/SDDM/...), so make sure the package really got installed.
if not test -f /usr/share/wayland-sessions/ymir.desktop
    echo "error: ymir.desktop is missing from /usr/share/wayland-sessions; the install appears to have failed." >&2
    exit 1
end

set CONFIG_DIR "$HOME/.config/ymir"
if set -q XDG_CONFIG_HOME
    set CONFIG_DIR "$XDG_CONFIG_HOME/ymir"
end
set CONFIG_FILE "$CONFIG_DIR/config.kdl"
if not test -e "$CONFIG_FILE"
    echo ">>> Seeding $CONFIG_FILE with the dwindle example config"
    mkdir -p "$CONFIG_DIR"
    cp "$YMIR_REPO_DIR/resources/dwindle-config.kdl" "$CONFIG_FILE"
end

echo
echo "ymir installed. Next steps:"
echo "  1. At the login screen (GDM/SDDM/...), pick the \"Ymir\" session."
echo "     (The session entry is installed at /usr/share/wayland-sessions/ymir.desktop.)"
echo "  2. Or, from a TTY, log in and run: ymir-session"
echo
echo "Dwindle test bindings (see the config for details):"
echo "  Mod+Shift+D      switch-column-display (dwindle <-> scrollable)"
echo "  Mod+Space        toggle-split"
echo "  Mod+Ctrl+Space   preselect \"bottom\""
echo "  Mod+Shift+Home   promote-window"
echo "  Mod+Comma        consume-window-into-column"
echo "  Mod+Period       expel-window-from-column"