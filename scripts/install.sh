#!/usr/bin/env bash
# ymir multi-distro installer.
#
# Two install modes (override with --binary / --source, or set YMIR_MODE):
#
#   binary (default)
#     - detects the distro (Arch, Fedora, Debian/Ubuntu, openSUSE) + arch
#     - installs the runtime dependencies for that distro
#     - downloads the NEWEST pre-built binary release for this distro+arch
#     - installs the binary, session script, greeter entry, and units
#
#   source
#     - detects the distro (same set)
#     - installs the build dependencies (plus Rust if missing)
#     - clones the newest main, builds with cargo build --release
#     - installs the result the same full way (greeter entry, units, seed config)
#
# Both modes run a shared "full install" step: the session is registered with the
# display manager (greeter), the systemd + dinit user units are installed, and
# ~/.config/ymir/init.lua is seeded with the dwindle example config if absent.
#
# Safe to re-run any time: binary mode always pulls the newest rolling release.
set -euo pipefail

REPO_URL="https://lab.braxton.onl/braxton/ymir"
# Forgejo's REST API base is <AppURL>/api/v1; the repo is a path segment added
# below. (Do not append the repo path to API_BASE — that produces a 404.)
API_BASE="https://lab.braxton.onl/api/v1"
SRC_REPO="${YMIR_SRC_REPO:-https://lab.braxton.onl/braxton/ymir.git}"
SRC_BRANCH="${YMIR_SRC_BRANCH:-main}"
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
# Mode selection (binary | source)
###############################################################################

MODE=""
for arg in "$@"; do
    case "$arg" in
        --binary) MODE="binary" ;;
        --source) MODE="source" ;;
        --help|-h)
            echo "usage: install.sh [--binary|--source]"
            echo "  --binary  download the newest pre-built binary release (default)"
            echo "  --source  build from source (newest main) on this distro"
            exit 0
            ;;
    esac
done

if [[ -z "$MODE" ]]; then
    MODE="${YMIR_MODE:-binary}"
fi
case "$MODE" in
    binary|source) ;;
    *) die "unknown install mode '$MODE'; choose 'binary' or 'source'." ;;
esac
log "Install mode: $MODE"

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
# Dependency installation
###############################################################################

install_runtime_deps() {
    case "$DISTRO" in
        arch)
            if ! have pacman; then die "detected Arch but pacman was not found."; fi
            local deps=( cairo fontconfig freetype2 harfbuzz libdrm
                         libdisplay-info libglvnd libinput libxcb libxkbcommon
                         mesa pango pipewire seatd wayland )
            log "Installing runtime dependencies via pacman"
            sudo pacman -S --needed --noconfirm "${deps[@]}"
            ;;
        fedora)
            if ! have dnf; then die "detected Fedora but dnf was not found."; fi
            local deps=( cairo-devel systemd-devel dbus-devel libdisplay-info-devel
                         libinput-devel libseat-devel libxkbcommon-devel mesa-libgbm-devel
                         mesa-libGL-devel mesa-libEGL-devel pango-devel wayland-devel
                         fontconfig-devel freetype-devel harfbuzz-devel
                         libxcb-devel pipewire-devel glib2-devel libdrm-devel
                         libwayland-client libwayland-server libxkbcommon libseat seatd pipewire )
            log "Installing runtime dependencies via dnf"
            sudo dnf install -y "${deps[@]}"
            ;;
        debian|ubuntu)
            if ! have apt-get; then die "detected Debian/Ubuntu but apt-get was not found."; fi
            local deps=( libcairo2-dev libsystemd-dev libdbus-1-dev libdisplay-info-dev
                         libinput-dev libseat-dev libxkbcommon-dev libgbm-dev
                         libegl1-mesa-dev mesa-common-dev libgl-dev libegl-dev
                         libpango1.0-dev libwayland-dev wayland-protocols
                         libfontconfig1-dev libfreetype-dev libharfbuzz-dev
                         libxcb1-dev libxcb-render0-dev libxcb-composite0-dev
                         libxcb-icccm4-dev libxcb-ewmh-dev libxcb-shape0-dev
                         libpipewire-0.3-dev libspa-0.2-dev libglib2.0-dev libdrm-dev
                         libudev-dev seatd pipewire curl ca-certificates )
            log "Installing runtime dependencies via apt"
            sudo apt-get update
            sudo apt-get install -y "${deps[@]}"
            ;;
        opensuse)
            if ! have zypper; then die "detected openSUSE but zypper was not found."; fi
            local deps=( systemd-devel dbus-1-devel libdisplay-info-devel libinput-devel
                         seatd-devel libxkbcommon-devel libgbm-devel Mesa-libEGL-devel
                         pango-devel wayland-devel wayland-protocols-devel fontconfig-devel
                         freetype2-devel harfbuzz-devel libxcb-devel pipewire-devel
                         glib2-devel libdrm-devel seatd curl tar )
            log "Installing runtime dependencies via zypper"
            sudo zypper --non-interactive install "${deps[@]}"
            ;;
    esac
}

install_build_deps() {
    # Same dependency sets the CI uses to compile against on each distro.
    case "$DISTRO" in
        arch)
            if ! have pacman; then die "detected Arch but pacman was not found."; fi
            local deps=( base-devel git pkgconf clang nodejs
                         cairo fontconfig freetype2 harfbuzz libdrm
                         libdisplay-info libglvnd libinput libxcb libxkbcommon
                         mesa pango pipewire seatd wayland wayland-protocols )
            log "Installing build dependencies via pacman"
            sudo pacman -S --needed --noconfirm "${deps[@]}"
            ;;
        fedora)
            if ! have dnf; then die "detected Fedora but dnf was not found."; fi
            local deps=( gcc make git pkgconf nodejs clang clang-libs
                         cairo-devel cairo-gobject-devel systemd-devel dbus-devel
                         libdisplay-info-devel libinput-devel libseat-devel
                         libxkbcommon-devel mesa-libgbm-devel mesa-libGL-devel
                         mesa-libEGL-devel pango-devel wayland-devel
                         wayland-protocols-devel fontconfig-devel freetype-devel
                         harfbuzz-devel libxcb-devel pipewire-devel glib2-devel
                         libdrm-devel )
            log "Installing build dependencies via dnf"
            sudo dnf install -y "${deps[@]}"
            ;;
        debian|ubuntu)
            if ! have apt-get; then die "detected Debian/Ubuntu but apt-get was not found."; fi
            local deps=( build-essential gcc make git pkgconf nodejs
                         libclang-dev clang
                         libcairo2-dev libsystemd-dev libdbus-1-dev libdisplay-info-dev
                         libinput-dev libseat-dev libxkbcommon-dev libgbm-dev
                         libegl1-mesa-dev mesa-common-dev libgl-dev libegl-dev
                         libpango1.0-dev libwayland-dev wayland-protocols
                         libfontconfig1-dev libfreetype-dev libharfbuzz-dev
                         libxcb1-dev libxcb-render0-dev libxcb-composite0-dev
                         libxcb-icccm4-dev libxcb-ewmh-dev libxcb-shape0-dev
                         libpipewire-0.3-dev libspa-0.2-dev libglib2.0-dev libdrm-dev
                         libudev-dev curl ca-certificates )
            log "Installing build dependencies via apt"
            sudo apt-get update
            sudo apt-get install -y "${deps[@]}"
            ;;
        opensuse)
            if ! have zypper; then die "detected openSUSE but zypper was not found."; fi
            local deps=( gcc make git curl tar nodejs-default clang libclang13
                         meson ninja
                         systemd-devel dbus-1-devel libdisplay-info-devel libinput-devel
                         libxkbcommon-devel libgbm-devel Mesa-libEGL-devel
                         pango-devel wayland-devel wayland-protocols-devel fontconfig-devel
                         freetype2-devel harfbuzz-devel libxcb-devel pipewire-devel
                         glib2-devel libdrm-devel )
            log "Installing build dependencies via zypper"
            sudo zypper --non-interactive install "${deps[@]}"
            ;;
    esac
}

ensure_rust() {
    if have cargo && have rustc; then
        log "Rust toolchain present ($(rustc --version | cut -d' ' -f2))"
        return
    fi
    log "Installing Rust via rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    # shellcheck disable=SC1090
    . "$HOME/.cargo/env"
    command -v cargo >/dev/null 2>&1 || die "cargo not found after installing rustup."
}

###############################################################################
# Shared full install.
# Expects a staging dir laid out exactly like the release tarball:
#   $1/ymir/{ymir,ymir-session,ymir.desktop,ymir-portals.conf,ymir.service,
#            ymir-shutdown.target,dinit-ymir,dinit-ymir.target,
#            default-config.lua,dwindle-config.lua}
###############################################################################

full_install() {
    local staging="$1"
    local app="$staging/ymir"
    [[ -d "$app" ]] || die "expected a 'ymir/' directory in staging, but it was not found."

    local missed=()
    for f in ymir ymir-session ymir.desktop ymir-portals.conf ymir.service \
             ymir-shutdown.target dinit-ymir dinit-ymir.target \
             default-config.lua dwindle-config.lua; do
        [[ -e "$app/$f" ]] || missed+=("$f")
    done
    if [[ "${#missed[@]}" -gt 0 ]]; then
        die "staging is missing files: ${missed[*]}"
    fi

    log "Installing ymir to $PREFIX"
    sudo install -Dm755 -t "$PREFIX/bin" "$app/ymir"
    sudo install -Dm755 -t "$PREFIX/bin" "$app/ymir-session"

    sudo install -Dm644 -t "$DESKTOP_DIR" "$app/ymir.desktop"
    sudo install -Dm644 -t /usr/share/xdg-desktop-portal "$app/ymir-portals.conf"

    sudo install -Dm644 -t /usr/lib/systemd/user "$app/ymir.service"
    sudo install -Dm644 -t /usr/lib/systemd/user "$app/ymir-shutdown.target"

    sudo install -Dm644 -t /etc/dinit.d/user/ymir "$app/dinit-ymir"
    sudo install -Dm644 -t /etc/dinit.d/user "$app/dinit-ymir.target"

    if [[ ! -f "$DESKTOP_DIR/ymir.desktop" ]]; then
        die "ymir.desktop is missing from $DESKTOP_DIR; the install appears to have failed."
    fi

    if [[ ! -e "$CONFIG_FILE" ]]; then
        log "Seeding $CONFIG_FILE with the dwindle example config"
        mkdir -p "$CONFIG_DIR"
        if [[ -f "$app/dwindle-config.lua" ]]; then
            cp "$app/dwindle-config.lua" "$CONFIG_FILE"
        else
            cp "$app/default-config.lua" "$CONFIG_FILE"
        fi
    fi
}

###############################################################################
# Binary mode: fetch the newest release asset for distro+arch and extract it.
###############################################################################

fetch_and_stage_binary() {
    have curl || die "curl is required but was not found."

    log "Fetching newest release info from $REPO_URL"
    # Get the newest release (by creation time) that has an asset for this
    # distro+arch, so we always install the newest pre-built binary, even if a
    # 'latest'-style tag happened to lag behind.
    #
    # The CI publishes one binary per distro it builds on (arch, fedora, debian,
    # opensuse). Ubuntu binaries are built from the Debian container, so look up
    # the Debian asset for Ubuntu releases.
    local asset_distro="$DISTRO"
    if [[ "$asset_distro" == "ubuntu" ]]; then
        asset_distro="debian"
    fi
    ASSET_NAME="ymir-${asset_distro}-${ARCH}.tar.gz"
    ASSET_URL=$(curl -sf -H "Accept: application/json" \
        "$API_BASE/repos/braxton/ymir/releases?limit=20" \
        | grep -o "\"browser_download_url\":\"[^\"]*${ASSET_NAME}\"" \
        | head -1 | sed 's/.*"browser_download_url":"//;s/"//') \
        || die "failed to fetch the latest release from $REPO_URL (is the CI running?)"

    if [[ -z "$ASSET_URL" ]]; then
        die "no binary found for distro=$DISTRO arch=$ARCH in the newest release. Available assets may not include this combination yet."
    fi

    log "Downloading $ASSET_NAME (newest release)"
    curl -sfL -o "$WORKDIR/$ASSET_NAME" "$ASSET_URL" \
        || die "failed to download $ASSET_URL"

    log "Extracting $ASSET_NAME"
    tar -xzf "$WORKDIR/$ASSET_NAME" -C "$WORKDIR"

    [[ -d "$WORKDIR/ymir" ]] || die "expected a 'ymir/' directory inside the tarball, but it was not found."
    STAGING="$WORKDIR"
}

###############################################################################
# Source mode: clone newest main, build, and stage the files.
###############################################################################

build_and_stage_source() {
    have git || die "git is required to build from source."
    ensure_rust

    log "Cloning newest $SRC_BRANCH from $SRC_REPO"
    git clone --depth 1 --branch "$SRC_BRANCH" "$SRC_REPO" "$WORKDIR/ymir-src" \
        || die "failed to clone $SRC_REPO"

    # Build only the compositor binary, locked to the committed Cargo.lock,
    # exactly as the CI (and the Arch PKGBUILD) do.
    log "Building ymir (cargo build --release --locked -p ymir --bin ymir)"
    if [[ "$DISTRO" == "opensuse" ]]; then
        # openSUSE does not ship libseat dev files; the build needs the
        # pkg-config path from a locally built libseat (as CI sets up).
        if [[ -d /usr/local/lib64/pkgconfig ]] || [[ -d /usr/local/lib/pkgconfig ]]; then
            export PKG_CONFIG_PATH="/usr/local/lib64/pkgconfig:/usr/local/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
        fi
        if ! command -v pkg-config >/dev/null 2>&1 || ! pkg-config --exists libseat 2>/dev/null; then
            log "Building libseat from source (openSUSE has no libseat dev package)"
            local seatd
            seatd=$(mktemp -d)
            git clone --depth 1 --branch 0.9.3 https://github.com/kennylevinsen/seatd.git "$seatd" >/dev/null 2>&1 \
                || die "failed to clone libseat/seatd source"
            ( cd "$seatd" \
                && meson setup build -Dserver=disabled -Dexamples=disabled -Dman-pages=disabled >/dev/null \
                && ninja -C build >/dev/null \
                && sudo meson install -C build >/dev/null )
            rm -rf "$seatd"
            export PKG_CONFIG_PATH="/usr/local/lib64/pkgconfig:/usr/local/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
        fi
    fi
    ( cd "$WORKDIR/ymir-src" && cargo build --release --locked -p ymir --bin ymir ) \
        || die "cargo build failed; see the build output above."

    # Stage in the same layout as the release tarball so the shared full
    # install step is identical to the binary path.
    local src="$WORKDIR/ymir-src"
    local app="$WORKDIR/ymir"
    mkdir -p "$app"
    install -Dm755 "$src/target/release/ymir"       "$app/ymir"
    install -Dm755 "$src/resources/ymir-session"    "$app/ymir-session"
    install -Dm644 "$src/resources/ymir.desktop"    "$app/ymir.desktop"
    install -Dm644 "$src/resources/ymir-portals.conf" "$app/ymir-portals.conf"
    install -Dm644 "$src/resources/ymir.service"    "$app/ymir.service"
    install -Dm644 "$src/resources/ymir-shutdown.target" "$app/ymir-shutdown.target"
    install -Dm644 "$src/resources/dinit/ymir"      "$app/dinit-ymir"
    install -Dm644 "$src/resources/dinit/ymir.target" "$app/dinit-ymir.target"
    install -Dm644 "$src/resources/default-config.lua" "$app/default-config.lua"
    install -Dm644 "$src/resources/dwindle-config.lua" "$app/dwindle-config.lua"

    STAGING="$WORKDIR"
}

###############################################################################
# Run
###############################################################################

# Stage on the home filesystem, not /tmp: /tmp is often a small tmpfs that can
# be mounted with a per-user quota (usrquota) and fill up — writing the ~10MB
# binary then fails with "Disk quota exceeded" even though space looks free.
# Verify the staged dir can actually take the binary's worth of bytes before
# committing to it, and fall back to a home-based dir if it cannot.

pick_workdir() {
    local base="${YMIR_TMPDIR:-${TMPDIR:-/tmp}}"
    # Prefer $YMIR_TMPDIR if set; otherwise honor the quota: avoid any temp
    # mount with a user/group quota and just use $HOME.
    if [[ -z "${YMIR_TMPDIR:-}" ]] &&
       mount | awk '$3 == "'"$base"'" && ($6 ~ /usrquota/ || $6 ~ /grpquota/) {found=1} END {exit !found}'; then
        rm -rf "$WORKDIR" 2>/dev/null || true
        WORKDIR="$(mktemp -d "$HOME/.ymir-install.XXXXXX")"
        return
    fi

    WORKDIR="$(mktemp -d "$base/ymir-install.XXXXXX")"
    # Write a real multi-megabyte probe: a 0-byte touch succeeds even when the
    # dir is over its quota, so it can't catch the failure we actually hit.
    if ! dd if=/dev/zero of="$WORKDIR/.probe" bs=1M count=16 2>/dev/null; then
        rm -rf "$WORKDIR"
        WORKDIR="$(mktemp -d "$HOME/.ymir-install.XXXXXX")"
    else
        rm -f "$WORKDIR/.probe"
    fi
}

WORKDIR=""
pick_workdir
trap 'rm -rf "$WORKDIR"' EXIT

if [[ "$MODE" == "source" ]]; then
    install_build_deps
    build_and_stage_source
else
    install_runtime_deps
    fetch_and_stage_binary
fi

full_install "$STAGING"

echo
echo "ymir installed ($MODE mode; distro: $DISTRO, arch: $ARCH). Next steps:"
echo "  1. At the login screen (GDM/SDDM/...), pick the \"Ymir\" session."
echo "     (Greeter entry: $DESKTOP_DIR/ymir.desktop.)"
echo "  2. Or, from a TTY, log in and run: ymir-session"
echo
if [[ "$MODE" == "binary" ]]; then
    echo "Re-running this script in binary mode pulls the newest pre-built release"
    echo "and reinstalls, so use it to update to the latest ymir."
else
    echo "Re-running this script in source mode clones the newest main, rebuilds,"
    echo "and reinstalls."
fi
echo
echo "Dwindle bindings from the seeded config:"
echo "  Mod+Shift+D      switch-column-display (dwindle <-> scrollable)"
echo "  Mod+Space        toggle-split"
echo "  Mod+Ctrl+Space   preselect \"bottom\""
echo "  Mod+Shift+Home   promote-window"
echo "  Mod+Comma        consume-window-into-column"
echo "  Mod+Period       expel-window-from-column"
