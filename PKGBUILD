# Maintainer: CachyOS Explorer Contributors
pkgname=cachyos-explorer
pkgver=0.1.0
pkgrel=1
pkgdesc="Explorador de archivos de alto rendimiento en Rust para GNOME (CachyOS)"
arch=('x86_64' 'x86_64_v3' 'x86_64_v4' 'aarch64')
url="https://github.com/cachyos/explorer"
license=('GPL-3.0-or-later')
depends=('gtk4' 'libadwaita' 'glibc' 'gcc-libs')
makedepends=('cargo' 'pkgconf')
source=()

build() {
  cd "${startdir}"
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR=target
  export RUSTFLAGS="-C target-cpu=x86-64-v3"
  cargo build --frozen --release
}

package() {
  cd "${startdir}"
  install -Dm755 "target/release/explorer" "${pkgdir}/usr/bin/explorer"
  install -Dm644 "data/org.cachyos.explorer.desktop" \
    "${pkgdir}/usr/share/applications/org.cachyos.explorer.desktop"
  install -Dm644 "data/icons/folder-cyan.svg" \
    "${pkgdir}/usr/share/icons/hicolor/scalable/apps/org.cachyos.explorer.svg"
}
