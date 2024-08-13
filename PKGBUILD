# Maintainer: Arnau Abella <arnau.abella@monadplus.pro>

pkgname=elgato-keylight
pkgver=0.5.0
pkgrel=1
pkgdesc="An Elgato Key Light Controller GUI"
arch=('x86_64')
url="https://github.com/monadplus/elgato-keylight"
license=('MIT')
depends=('gcc-libs' 'pango' 'cairo' 'glib2' 'glibc' 'openssl' 'avahi' 'gtk3' 'gdk-pixbuf2' 'xdotool' 'libappindicator-gtk3')
makedepends=('cargo')
source=("$pkgname-$pkgver.tar.gz::$url/archive/$pkgver.tar.gz")
sha512sums=('b821f2e1b1436cc7de6124d5a8408ad94c880a9c36fe04461f8ab31126578979cb8cdfd3124326be136686b041863cde16ce2683f6d88af7961a376b2349a400')

prepare() {
  cd "$pkgname-$pkgver" || exit 1
  cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
  cd "$pkgname-$pkgver" || exit 1
  CFLAGS+=' -ffat-lto-objects' # hidden symbol `ring_core_0_17_8_OPENSSL_ia32cap_P' isn't define
  local _features="--features=tray-icon"
  cargo build --release --frozen --bin=$pkgname $_features
}

check() {
  cd "$pkgname-$pkgver" || exit 1
  # cargo test --frozen
}

package() {
  cd "$pkgname-$pkgver" || exit 1
  install -Dm 755 "target/release/$pkgname" -t "$pkgdir/usr/bin"
  install -Dm 644 README.md -t "$pkgdir/usr/share/doc/$pkgname"
  install -Dm 644 LICENSE -t "$pkgdir/usr/share/licenses/$pkgname"
}
