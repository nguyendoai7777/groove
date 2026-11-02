#!/usr/bin/env bash
# Dựng `<out>/groove-x_<version>_<arch>.deb` (groove-gd1_27, gd2_01). Chạy trên Ubuntu / Debian.
#   sudo apt install libasound2-dev dpkg-dev
#   bash packaging/linux/build-deb.sh                  # bản phát hành: thiếu trang giấy phép là dừng
#   bash packaging/linux/build-deb.sh --draft          # bản thử: thiếu thì cảnh báo, tên có `-draft`
# Thêm: --skip-build, --out <thư mục> (mặc định dist/linux).
#
# G-2: `libasound` (LGPL) chỉ link động, KHÔNG chép vào gói, khai ở `Depends`. Script dừng nếu thấy nó trong gói hoặc exe không
# cần nó. Phụ thuộc còn lại lấy từ `dpkg-shlibdeps` theo đúng thư viện exe cần. Không có maintainer script: trigger của
# `desktop-file-utils` và `hicolor-icon-theme` tự cập nhật liên kết MIME và icon khi cài, gỡ.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
draft=""
skip_build=""
out=""
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out="$2"; shift 2 ;;
    --draft) draft=1; shift ;;
    --skip-build) skip_build=1; shift ;;
    *) echo "Tham số lạ: $1" >&2; exit 2 ;;
  esac
done
[ "$(uname -s)" = Linux ] || { echo "Chỉ chạy trên Linux." >&2; exit 1; }
for tool in dpkg-deb dpkg-shlibdeps dpkg-architecture; do
  command -v "$tool" >/dev/null 2>&1 || { echo "Thiếu $tool: sudo apt install dpkg-dev" >&2; exit 1; }
done

# `tr`: checkout từ Windows có thể là CRLF, `"$` không khớp.
version="$(tr -d '\r' < "$repo/Cargo.toml" | sed -n 's/^version = "\(.*\)"$/\1/p' | head -1)"
[ -n "$version" ] || { echo "Không đọc được version trong Cargo.toml" >&2; exit 1; }
arch="$(dpkg-architecture -qDEB_HOST_ARCH)"
target_dir="${CARGO_TARGET_DIR:-$repo/target}"
exe="$target_dir/release/groove"

if [ -z "$skip_build" ]; then
  (cd "$repo" && cargo build --release --locked -p groove-app --bin groove)
fi
objdump -p "$exe" | grep -q 'NEEDED.*libasound\.so\.2' || { echo "exe không link động libasound (G-2)" >&2; exit 1; }

[ -n "$out" ] || out="$repo/dist/linux"
mkdir -p "$out"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
root="$work/root"
mkdir -p "$root/DEBIAN" "$root/usr/bin" "$root/usr/share/applications" "$root/usr/share/doc/groove-x"
install -m 0755 "$exe" "$root/usr/bin/groove"
install -m 0644 "$repo/packaging/linux/groove.desktop" "$root/usr/share/applications/groove.desktop"
# Icon hicolor mọi cỡ trong `packaging/linux/icons/<cỡ>.png` (gd2_01): menu, "Mở bằng", thanh tác vụ không phải co ảnh 256.
for icon in "$repo"/packaging/linux/icons/*x*.png; do
  dim="$(basename "$icon" .png)"
  install -D -m 0644 "$icon" "$root/usr/share/icons/hicolor/$dim/apps/groove.png"
done

warnings=""
if command -v pwsh >/dev/null 2>&1 && command -v cargo-about >/dev/null 2>&1; then
  pwsh -NoProfile -File "$repo/packaging/licenses/gen-licenses.ps1" -Out "$root/usr/share/doc/groove-x"
elif [ -n "$draft" ]; then
  warnings="⚠ Không có trang giấy phép: cần pwsh + cargo-about."$'\n'
else
  echo "Không có trang giấy phép: cần pwsh + cargo-about (bản thử: --draft)." >&2
  exit 1
fi
cat > "$root/usr/share/doc/groove-x/copyright" <<'EOF'
Groove X is proprietary software. Third-party licenses: /usr/share/doc/groove-x/third-party.html
The ALSA library (libasound, LGPL-2.1-or-later) is not part of this package: it is a dependency installed by the system.
EOF

if find "$root" -name 'libasound*' | grep -q .; then
  echo "libasound không được nằm trong gói (G-2)" >&2
  exit 1
fi

# `dpkg-shlibdeps` đòi `debian/control` ở thư mục đang đứng, nội dung không dùng tới.
mkdir -p "$work/debian"
: > "$work/debian/control"
shlibs="$(cd "$work" && dpkg-shlibdeps -O "root/usr/bin/groove" 2>/dev/null | sed -n 's/^shlibs:Depends=//p')"
# Ubuntu 24.04 đổi tên thành `libasound2t64`; `dpkg-shlibdeps` chỉ ghi tên của máy dựng nên khai cả hai, rồi bỏ dòng của nó.
depends="$(printf '%s' "$shlibs" | tr ',' '\n' | sed 's/^ *//' | grep -v '^libasound2' | paste -sd, - | sed 's/,/, /g')"
depends="libasound2 | libasound2t64${depends:+, $depends}"

size="$(du -sk "$root/usr" | cut -f1)"
cat > "$root/DEBIAN/control" <<EOF
Package: groove-x
Version: $version
Section: sound
Priority: optional
Architecture: $arch
Maintainer: Groove X <noreply@groove-x.invalid>
Installed-Size: $size
Depends: $depends
Recommends: desktop-file-utils, hicolor-icon-theme
Description: Groove X music player
 Plays and organizes your local music library.
EOF

suffix=""
[ -z "$draft" ] || suffix="-draft"
deb="$out/groove-x_${version}_${arch}${suffix}.deb"
dpkg-deb --build --root-owner-group "$root" "$deb"
printf '\nXong: %s (%s)\nDepends: %s\n%s' "$deb" "$(du -h "$deb" | cut -f1)" "$depends" "$warnings"
