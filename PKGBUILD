# Maintainer: braxton <5raxton@users.noreply.github.com>
# Preview: makepkg --printsrcinfo

pkgname=ymir
pkgdesc="A scrollable-tiling Wayland compositor, fork of niri with dwindle column mode"
# Placeholder; the pkgver() function below overrides it with the git version at build time.
pkgver=26.4.0
pkgver() {
    if git -C "$srcdir/$pkgname" describe --always --tags >/dev/null 2>&1; then
        git -C "$srcdir/$pkgname" describe --always --tags | sed 's/^v//; s/-/./g'
    else
        # Fallback so that e.g. `makepkg --printsrcinfo` works without fetching the source.
        echo "26.4.0"
    fi
}
pkgrel=1
arch=('x86_64')
url="https://github.com/5raxton/ymir"
license=('GPL-3.0-or-later')
depends=(
    'cairo'
    'fontconfig'
    'freetype2'
    'harfbuzz'
    'libdrm'
    'libdisplay-info'
    'libglvnd'
    'libinput'
    'libxcb'
    'libxkbcommon'
    'mesa'
    'pango'
    'pipewire'
    'seatd'
    'wayland'
)
makedepends=(
    'base-devel'
    'git'
    'pkgconf'
    'rust'
    'wayland-protocols'
)
optdepends=(
    'alacritty: default terminal spawned by the example config'
    'fuzzel: default launcher spawned by the example config'
    'kanshi: monitor configuration'
    'pipewire: audio support in portals'
    'wireplumber: audio session manager'
    'xdg-desktop-portal: system portals (required for screenshots and screencasting)'
    'xdg-desktop-portal-gnome: screencast/screenshot portals'
    'xdg-desktop-portal-gtk: file chooser portal'
    'ydotool: synthetic input for night manipulation scripts'
)
source=("$pkgname::git+ssh://git@github.com/5raxton/ymir.git#branch=main")
sha256sums=('SKIP')

build() {
    cd "$pkgname"
    cargo build --release
}

package() {
    cd "$pkgname"

    install -Dm755 target/release/ymir -t "$pkgdir/usr/bin"
    install -Dm755 resources/ymir-session -t "$pkgdir/usr/bin"

    # The .desktop file is picked up by login managers (GDM, SDDM, ...) so that
    # "Ymir" shows up as a selectable session in the greeter.
    install -Dm644 resources/ymir.desktop -t "$pkgdir/usr/share/wayland-sessions"

    install -Dm644 resources/ymir-portals.conf -t "$pkgdir/usr/share/xdg-desktop-portal"

    # systemd user units, started by the ymir-session wrapper script.
    install -Dm644 resources/ymir.service -t "$pkgdir/usr/lib/systemd/user"
    install -Dm644 resources/ymir-shutdown.target -t "$pkgdir/usr/lib/systemd/user"

    # dinit user units, used when starting via dinit instead of systemd.
    install -Dm644 resources/dinit/ymir -t "$pkgdir/etc/dinit.d/user/ymir"
    install -Dm644 resources/dinit/ymir.target -t "$pkgdir/etc/dinit.d/user"
}