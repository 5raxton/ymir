#!/usr/bin/env bash
# ymir multi-distro installer.
#
#   - detects the distro (Arch, Fedora, Debian/Ubuntu, openSUSE)
#   - installs the runtime dependencies for that distro
#   - downloads the latest pre-built binary from the rolling release
#   - installs the binary, session script, and supporting files
#   - seeds ~/.config/ymir/init.lua with the dwindle example config if absent
#
# Safe to re-run any time: each run pulls the latest rolling release and
# reinstalls, so it also serves as a full update.
set -euo pipefail

REPO_URL="https://lab.braxton.onl/braxton/ymir"
API_BASE="$REPO_URL/api/v1"
PREFIX="${YMIR_PREFIX:-/usr/local}"
DESKTOP_DIR="${YMIR_DESKTOP_DIR:-/usr/share/wayland-sessions}"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/ymir"
CONFIG_FILE="$CONFIG_DIR/init.lua"

log()  { printf '\033[1;34m>>>\033[0m %s\n' "$*"; }
die()  { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

if [[ "$(id -u)" -eq 0 ]]; then
    die "do not run this script as root; run it as a regular user (sudo is used for the privileged steps)."
fi

###############################################################################
# Distro detection
###############################################################################

detect_distro() {
    if [[ -n "${YMIR_DISTRO:-}" ]]; then
        echo "$YMIR_DISTRO"
        return
    fi
    local id="" id_like=""
    if [[ -r /etc/os-release ]]; then
        # shellcheck disable=SC1091
        . /etc/os-release
        id="${ID:-}"
        id_like="${ID_LIKE:-}"
    fi

    if [[ "$id" == opensuse* ]] || [[ "$id_like" == *suse* ]]; then
        echo "opensuse"; return
    fi
    case "$id" in
        arch)   echo "arch"; return ;;
        fedora) echo "fedora"; return ;;
        debian) echo "debian"; return ;;
        ubuntu) echo "ubuntu"; return ;;
    esac
    case "$id_like" in
        *arch*)   echo "arch"; return ;;
        *fedora*) echo "fedora"; return ;;
        *debian*) echo "debian"; return ;;
        *ubuntu*) echo "ubuntu"; return ;;
    esac

    die "unsupported distro (ID=$id ID_LIKE=$id_like). Supported: Arch, Fedora, Debian/Ubuntu, openSUSE."
}

DISTRO="$(detect_distro)"
log "Detected distro: $DISTRO"

###############################################################################
# Architecture detection
###############################################################################

detect_arch() {
    local machine
    machine="$(uname -m)"
    case "$machine" in
        x86_64)  echo "x86_64" ;;
        aarch64) echo "arm64" ;;
        *)       die "unsupported architecture: $machine" ;;
    esac
}

ARCH="$(detect_arch)"
log "Detected arch: $ARCH"

###############################################################################
# Runtime dependency installation
###############################################################################

case "$DISTRO" in
    arch)
        if ! have pacman; then
            die "detected Arch but pacman was not found."
        fi
        DEPENDENCIES=(
            cairo fontconfig freetype2 harfbuzz libdrm
            libdisplay-info libglvnd libinput libxcb libxkbcommon
            mesa pango pipewire seatd wayland
        )
        log "Installing runtime dependencies via pacman"
        sudo pacman -S --needed --noconfirm "${DEPENDENCIES[@]}"
        ;;
    fedora)
        if ! have dnf; then
            die "detected Fedora but dnf was not found."
        fi
        DEPENDENCIES=(
            cairo-devel systemd-devel dbus-devel libdisplay-info-devel
            libinput-devel libseat-devel libxkbcommon-devel mesa-libgbm-devel
            mesa-libGL-devel mesa-libEGL-devel pango-devel wayland-devel
            fontconfig-devel freetype-devel harfbuzz-devel
            libxcb-devel pipewire-devel glib2-devel libdrm-devel
            libwayland-client libwayland-server libxkbcommon libseat seatd pipewire
        )
        log "Installing runtime dependencies via dnf"
        sudo dnf install -y "${DEPENDENCIES[@]}"
        ;;
    debian|ubuntu)
        if ! have apt-get; then
            die "detected Debian/Ubuntu but apt-get was not found."
        fi
        DEPENDENCIES=(
            libcairo2-dev libsystemd-dev libdbus-1-dev libdisplay-info-dev
            libinput-dev libseat-dev libxkbcommon-dev libgbm-dev
            libegl1-mesa-dev mesa-common-dev libgl-dev libegl-dev
            libpango1.0-dev libwayland-dev wayland-protocols
            libfontconfig1-dev libfreetype-dev libharfbuzz-dev
            libxcb1-dev libxcb-render0-dev libxcb-composite0-dev
            libxcb-icccm4-dev libxcb-ewmh-dev libxcb-shape0-dev
            libpipewire-0.3-dev libspa-0.2-dev libglib2.0-dev libdrm-dev
            libudev-dev seatd pipewire curl ca-certificates
        )
        log "Installing runtime dependencies via apt"
        sudo apt-get update
        sudo apt-get install -y "${DEPENDENCIES[@]}"
        ;;
    opensuse)
        if ! have zypper; then
            die "detected openSUSE but zypper was not found."
        fi
        DEPENDENCIES=(
            systemd-devel dbus-1-devel libdisplay-info-devel libinput-devel
            seatd-devel libxkbcommon-devel libgbm-devel Mesa-libEGL-devel
            pango-devel wayland-devel wayland-protocols-devel fontconfig-devel
            freetype2-devel harfbuzz-devel libxcb-devel pipewire-devel
            glib2-devel libdrm-devel seatd curl tar
        )
        log "Installing runtime dependencies via zypper"
        sudo zypper --non-interactive install "${DEPENDENCIES[@]}"
        ;;
esac

###############################################################################
# Download latest pre-built binary
###############################################################################

have curl || die "curl is required but was not found."

log "Fetching latest release info from $REPO_URL"
RELEASE_JSON=$(curl -sf -H "Accept: application/json" \
    "$API_BASE/repos/braxton/ymir/releases/tags/latest") \
    || die "failed to fetch the latest release from $REPO_URL (is the CI running?)"

ASSET_NAME="ymir-${DISTRO}-${ARCH}.tar.gz"
ASSET_URL=$(echo "$RELEASE_JSON" | grep -o "\"browser_download_url\":\"[^\"]*${ASSET_NAME}\"" | head -1 | sed 's/.*"browser_download_url":"//;s/"//')

if [[ -z "$ASSET_URL" ]]; then
    die "no binary found for distro=$DISTRO arch=$ARCH in the latest release. Available assets may not include this combination yet."
fi

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

log "Downloading $ASSET_NAME"
curl -sfL -o "$TMPDIR/$ASSET_NAME" "$ASSET_URL" \
    || die "failed to download $ASSET_URL"

log "Extracting $ASSET_NAME"
tar -xzf "$TMPDIR/$ASSET_NAME" -C "$TMPDIR"

EXTRACTED="$TMPDIR/ymir"
if [[ ! -d "$EXTRACTED" ]]; then
    die "expected a 'ymir/' directory inside the tarball, but it was not found."
fi

###############################################################################
# Install
###############################################################################

log "Installing ymir to $PREFIX"
sudo install -Dm755 -t "$PREFIX/bin" "$EXTRACTED/ymir"
sudo install -Dm755 -t "$PREFIX/bin" "$EXTRACTED/ymir-session"

sudo install -Dm644 -t "$DESKTOP_DIR" "$EXTRACTED/ymir.desktop"
sudo install -Dm644 -t /usr/share/xdg-desktop-portal "$EXTRACTED/ymir-portals.conf"

sudo install -Dm644 -t /usr/lib/systemd/user "$EXTRACTED/ymir.service"
sudo install -Dm644 -t /usr/lib/systemd/user "$EXTRACTED/ymir-shutdown.target"

sudo install -Dm644 -t /etc/dinit.d/user/ymir "$EXTRACTED/dinit-ymir"
sudo install -Dm644 -t /etc/dinit.d/user "$EXTRACTED/dinit-ymir.target"

###############################################################################
# Desktop entry & default config
###############################################################################

if [[ ! -f "$DESKTOP_DIR/ymir.desktop" ]]; then
    die "ymir.desktop is missing from $DESKTOP_DIR; the install appears to have failed."
fi

if [[ ! -e "$CONFIG_FILE" ]]; then
    log "Seeding $CONFIG_FILE with the dwindle example config"
    mkdir -p "$CONFIG_DIR"
    if [[ -f "$EXTRACTED/dwindle-config.lua" ]]; then
        cp "$EXTRACTED/dwindle-config.lua" "$CONFIG_FILE"
    else
        cp "$EXTRACTED/default-config.lua" "$CONFIG_FILE"
    fi
fi

###############################################################################
# Done
###############################################################################

echo
echo "ymir installed (distro: $DISTRO, arch: $ARCH). Next steps:"
echo "  1. At the login screen (GDM/SDDM/...), pick the \"Ymir\" session."
echo "     (Session entry: $DESKTOP_DIR/ymir.desktop.)"
echo "  2. Or, from a TTY, log in and run: ymir-session"
echo
echo "Re-running this script pulls the latest build and reinstalls, so use it"
echo "to update to the latest ymir."
echo
echo "Dwindle bindings from the seeded config:"
echo "  Mod+Shift+D      switch-column-display (dwindle <-> scrollable)"
echo "  Mod+Space        toggle-split"
echo "  Mod+Ctrl+Space   preselect \"bottom\""
echo "  Mod+Shift+Home   promote-window"
echo "  Mod+Comma        consume-window-into-column"
echo "  Mod+Period       expel-window-from-column"
