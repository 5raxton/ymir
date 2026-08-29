#!/usr/bin/env bash
# ymir Arch Linux installer.
#
#   - installs the build/runtime dependencies from pacman
#   - clones (or updates) the repo from origin main
#   - builds and installs ymir via makepkg -si
#   - seeds ~/.config/ymir/config.kdl with the dwindle example config if absent
#
# Safe to re-run any time to pull updates from main and reinstall.
set -euo pipefail

REPO_URL="git@github.com:5raxton/ymir.git"
REPO_DIR="${YMIR_REPO_DIR:-$HOME/src/ymir}"
BRANCH="${YMIR_BRANCH:-main}"

# makepkg refuses to run as root, so make sure we're an unprivileged user here.
if [[ "$(id -u)" -eq 0 ]]; then
    echo "error: do not run this script as root; makepkg must run as a regular user." >&2
    exit 1
fi

if ! command -v pacman >/dev/null 2>&1; then
    echo "error: this installer targets Arch Linux (pacman was not found)." >&2
    exit 1
fi

# Must be a full pacman mirror of PKGBUILD makedepends.
DEPS=(
    base-devel
    cairo
    fontconfig
    freetype2
    git
    harfbuzz
    libdrm
    libdisplay-info
    libglvnd
    libinput
    libseat
    libxcb
    libxkbcommon
    mesa
    pango
    pkgconf
    pipewire
    rust
    wayland
    wayland-protocols
)

echo ">>> Installing build/runtime dependencies"
sudo pacman -S --needed --noconfirm "${DEPS[@]}"

mkdir -p "$(dirname "$REPO_DIR")"
if [[ ! -d "$REPO_DIR/.git" ]]; then
    echo ">>> Cloning ymir into $REPO_DIR"
    git clone --branch "$BRANCH" "$REPO_URL" "$REPO_DIR"
else
    echo ">>> Updating ymir in $REPO_DIR"
    # makepkg rewrites the PKGBUILD's pkgver line when building, so throw any
    # such local edits away before pulling.
    git -C "$REPO_DIR" fetch origin "$BRANCH"
    git -C "$REPO_DIR" reset --hard "origin/$BRANCH"
fi

echo ">>> Building and installing ymir (release build, this takes a while)"
pushd "$REPO_DIR" >/dev/null
makepkg -si --noconfirm --needed
popd >/dev/null

CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/ymir"
CONFIG_FILE="$CONFIG_DIR/config.kdl"
if [[ ! -e "$CONFIG_FILE" ]]; then
    echo ">>> Seeding $CONFIG_FILE with the dwindle example config"
    mkdir -p "$CONFIG_DIR"
    cp "$REPO_DIR/resources/dwindle-config.kdl" "$CONFIG_FILE"
fi

echo
echo "ymir installed. Next steps:"
echo "  1. At the login screen (GDM/SDDM/...), pick the \"Ymir\" session."
echo "     (The session entry is installed at /usr/share/wayland-sessions/ymir.desktop.)"
echo "  2. Or, from a TTY, log in and run: ymir-session"
echo
echo "Dwindle test bindings (see the config for details):"
echo "  Mod+Space        toggle-split"
echo "  Mod+Ctrl+Space   preselect \"bottom\""
echo "  Mod+Shift+Home   promote-window"
echo "  Mod+Comma        consume-window-into-column"
echo "  Mod+Period       expel-window-from-column"