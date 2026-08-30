#!/usr/bin/env bash
# ymir multi-distro installer.
#
#   - detects the distro (Arch, Fedora, Debian/Ubuntu, openSUSE)
#   - installs the build/runtime dependencies for that distro
#   - clones (or updates) the repo from origin main
#   - cleans stale build/download artifacts from previous runs
#   - builds and installs ymir (makepkg on Arch, cargo + /usr/local elsewhere)
#   - installs the session .desktop file and ports/config units automatically
#   - seeds ~/.config/ymir/config.kdl with the dwindle example config if absent
#
# Safe to re-run any time: each run pulls the latest main, cleans stale
# artifacts, and does a fresh build, so it also serves as a full update.
set -euo pipefail

REPO_URL="https://github.com/5raxton/ymir.git"
REPO_DIR="${YMIR_REPO_DIR:-$HOME/src/ymir}"
BRANCH="${YMIR_BRANCH:-main}"
PREFIX="${YMIR_PREFIX:-/usr/local}"
DESKTOP_DIR="${YMIR_DESKTOP_DIR:-/usr/share/wayland-sessions}"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/ymir"
CONFIG_FILE="$CONFIG_DIR/config.kdl"

log()  { printf '\033[1;34m>>>\033[0m %s\n' "$*"; }
die()  { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

# makepkg refuses to run as root; non-Arch cargo builds can run anywhere, but
# we keep a single requirement so one code path works everywhere.
if [[ "$(id -u)" -eq 0 ]]; then
    die "do not run this script as root; run it as a regular user (sudo is used for the privileged steps)."
fi

###############################################################################
# Distro detection
###############################################################################

detect_distro() {
    # Allow forcing the distro (e.g. on a derivative or for testing).
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

    # Normalize SUSE ids to a single "suse" bucket.
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
# Dependency installation
###############################################################################

case "$DISTRO" in
    arch)
        # makepkg makedepends + runtime libs, from the PKGBUILD.
        if ! have pacman; then
            die "detected Arch but pacman was not found."
        fi
        DEPENDENCIES=(
            base-devel
            cairo fontconfig freetype2 git harfbuzz libdrm
            libdisplay-info libglvnd libinput libxcb libxkbcommon
            mesa pango pkgconf pipewire rust seatd wayland
            wayland-protocols
        )
        log "Installing dependencies via pacman"
        sudo pacman -S --needed --noconfirm "${DEPENDENCIES[@]}"
        ;;
    fedora)
        if ! have dnf; then
            die "detected Fedora but dnf was not found."
        fi
        DEPENDENCIES=(
            gcc make pkgconf rust cargo
            cairo-devel systemd-devel dbus-devel libdisplay-info-devel
            libinput-devel libseat-devel libxkbcommon-devel mesa-libgbm-devel
            mesa-libGL-devel mesa-libEGL-devel pango-devel wayland-devel
            wayland-protocols-devel fontconfig-devel freetype-devel
            harfbuzz-devel libxcb-devel pipewire-devel glib2-devel libdrm-devel
            libwayland-client libwayland-server libxkbcommon libseat seatd pipewire
        )
        log "Installing dependencies via dnf"
        sudo dnf install -y "${DEPENDENCIES[@]}"
        if ! have git; then
            sudo dnf install -y git
        fi
        ;;
    debian|ubuntu)
        if ! have apt-get; then
            die "detected Debian/Ubuntu but apt-get was not found."
        fi
        if [[ "$DISTRO" == "ubuntu" ]]; then
            PKG_CONFIG_PKG="pkg-config"
        else
            PKG_CONFIG_PKG="pkgconf"
        fi
        DEPENDENCIES=(
            build-essential gcc make cargo rustc "$PKG_CONFIG_PKG"
            libcairo2-dev libsystemd-dev libdbus-1-dev libdisplay-info-dev
            libinput-dev libseat-dev libxkbcommon-dev libgbm-dev
            libegl1-mesa-dev mesa-common-dev libgl-dev libegl-dev
            libpango1.0-dev libwayland-dev wayland-protocols
            libfontconfig1-dev libfreetype-dev libharfbuzz-dev
            libxcb1-dev libxcb-render0-dev libxcb-composite0-dev
            libxcb-icccm4-dev libxcb-ewmh-dev libxcb-shape0-dev
            libpipewire-0.3-dev libspa-0.2-dev libglib2.0-dev libdrm-dev
            libudev-dev
            seatd pipewire git
        )
        log "Installing dependencies via apt"
        sudo apt-get update
        sudo apt-get install -y "${DEPENDENCIES[@]}"
        ;;
    opensuse)
        if ! have zypper; then
            die "detected openSUSE but zypper was not found."
        fi
        DEPENDENCIES=(
            gcc make rust cargo pkgconf pkgconf-pkg-config
            systemd-devel dbus-1-devel libdisplay-info-devel libinput-devel
            seatd-devel libxkbcommon-devel libgbm-devel Mesa-libEGL-devel
            pango-devel wayland-devel wayland-protocols-devel fontconfig-devel
            freetype2-devel harfbuzz-devel libxcb-devel pipewire-devel
            glib2-devel libdrm-devel seatd git curl
        )
        log "Installing dependencies via zypper"
        sudo zypper --non-interactive install "${DEPENDENCIES[@]}"
        ;;
esac

###############################################################################
# Clone / update
###############################################################################

have git || die "git was not found; it should have been installed with the dependencies above."

mkdir -p "$(dirname "$REPO_DIR")"
if [[ ! -d "$REPO_DIR/.git" ]]; then
    log "Cloning ymir into $REPO_DIR"
    git clone --branch "$BRANCH" "$REPO_URL" "$REPO_DIR"
else
    log "Updating ymir in $REPO_DIR to origin/$BRANCH"
    # Reuse $REPO_URL (HTTPS) regardless of what the clone's origin is set to:
    # a stale SSH URL from an earlier installer run would otherwise fail for
    # users without SSH keys.
    git -C "$REPO_DIR" remote set-url origin "$REPO_URL"
    # makepkg rewrites the PKGBUILD's pkgver line when building, so throw any
    # such local edits away before pulling.
    git -C "$REPO_DIR" fetch origin "$BRANCH"
    git -C "$REPO_DIR" reset --hard "origin/$BRANCH"
fi

###############################################################################
# Build & install
###############################################################################

if [[ "$DISTRO" == "arch" ]]; then
    log "Building and installing ymir (makepkg, release)"
    pushd "$REPO_DIR" >/dev/null

    # Auto-clean stale build/download artifacts so each run is a fresh,
    # bleeding-edge build: extracted srcdir, staging dir, and built packages.
    log "Cleaning stale makepkg artifacts"
    rm -rf "$REPO_DIR/src" "$REPO_DIR/pkg"
    rm -f "$REPO_DIR"/*.pkg.tar.zst "$REPO_DIR"/*.src.tar.gz 2>/dev/null || true

    # -C = cleanbuild (removes $srcdir), -c = clean pkgdir, -i = install,
    # --needed = skip already-installed deps.
    makepkg -Cci --noconfirm --needed
    popd >/dev/null
else
    log "Building and installing ymir (cargo, release)"
    pushd "$REPO_DIR" >/dev/null

    # Clean stale crates.io checkout + build artifacts from any previous run.
    log "Cleaning stale cargo artifacts"
    rm -rf "$REPO_DIR/target"

    # Build only the ymir binary package (not the workspace's visual tests,
    # which need gtk4/libadwaita dev libs we don't want to force).
    cargo build --release --locked -p ymir --bin ymir

    log "Installing to $PREFIX"
    sudo install -Dm755 -t "$PREFIX/bin" target/release/ymir
    sudo install -Dm755 -t "$PREFIX/bin" resources/ymir-session

    sudo install -Dm644 -t "$DESKTOP_DIR" resources/ymir.desktop
    sudo install -Dm644 -t /usr/share/xdg-desktop-portal resources/ymir-portals.conf

    # systemd user units (the default service manager on these distros).
    sudo install -Dm644 -t /usr/lib/systemd/user resources/ymir.service
    sudo install -Dm644 -t /usr/lib/systemd/user resources/ymir-shutdown.target

    # dinit user units, for users who run dinit instead of systemd.
    sudo install -Dm644 -t /etc/dinit.d/user/ymir resources/dinit/ymir
    sudo install -Dm644 -t /etc/dinit.d/user resources/dinit/ymir.target

    popd >/dev/null
fi

###############################################################################
# Desktop entry & default config
###############################################################################

# The .desktop file is what makes "Ymir" appear in the greeter (GDM/SDDM/...).
# Arch's PKGBUILD installs it; the non-Arch path above installs it too, so just
# verify it landed rather than moving anything by hand.
if [[ ! -f "$DESKTOP_DIR/ymir.desktop" ]]; then
    die "ymir.desktop is missing from $DESKTOP_DIR; the install appears to have failed."
fi

if [[ ! -e "$CONFIG_FILE" ]]; then
    log "Seeding $CONFIG_FILE with the dwindle example config"
    mkdir -p "$CONFIG_DIR"
    if [[ -f "$REPO_DIR/resources/dwindle-config.kdl" ]]; then
        cp "$REPO_DIR/resources/dwindle-config.kdl" "$CONFIG_FILE"
    else
        # Fall back to the default config if the dwindle example is absent.
        cp "$REPO_DIR/resources/default-config.kdl" "$CONFIG_FILE"
    fi
fi

###############################################################################
# Done
###############################################################################

echo
echo "ymir installed (distro: $DISTRO). Next steps:"
echo "  1. At the login screen (GDM/SDDM/...), pick the \"Ymir\" session."
echo "     (Session entry: $DESKTOP_DIR/ymir.desktop.)"
echo "  2. Or, from a TTY, log in and run: ymir-session"
echo
echo "Re-running this script pulls origin/$BRANCH, cleans old artifacts, and"
echo "reinstalls, so use it to update to the latest ymir."
echo
echo "Dwindle bindings from the seeded config:"
echo "  Mod+Shift+D      switch-column-display (dwindle <-> scrollable)"
echo "  Mod+Space        toggle-split"
echo "  Mod+Ctrl+Space   preselect \"bottom\""
echo "  Mod+Shift+Home   promote-window"
echo "  Mod+Comma        consume-window-into-column"
echo "  Mod+Period       expel-window-from-column"