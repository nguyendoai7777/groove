#!/usr/bin/env bash
# Dựng `<out>/Groovex.app` + `groovex-<version>-<arch>.dmg`, ký ad-hoc (groove-gd1_27). Chạy trên Mac. Không dùng Developer ID: đó là
# dịch vụ trả phí, chủ dự án quyết; lần đầu mở thì Gatekeeper cảnh báo, chuột phải > Open.
#   bash packaging/macos/build-macos.sh                # bản phát hành: thiếu trang giấy phép là dừng
#   bash packaging/macos/build-macos.sh --draft        # bản thử: thiếu thì cảnh báo, tên `.dmg` có `-draft`
# Thêm: --arch arm64|x86_64 (mặc định kiến trúc máy), --skip-build, --out <thư mục> (mặc định dist/macos-<arch>).
# Cần: `cargo install cargo-about --locked --features cli`, `pwsh` (`packaging/licenses/gen-licenses.ps1`).
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
arch="$(uname -m)"
draft=""
skip_build=""
out=""
while [ $# -gt 0 ]; do
  case "$1" in
    --arch) arch="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    --draft) draft=1; shift ;;
    --skip-build) skip_build=1; shift ;;
    *) echo "Tham số lạ: $1" >&2; exit 2 ;;
  esac
done
[ "$(uname -s)" = Darwin ] || { echo "Chỉ chạy trên macOS." >&2; exit 1; }
case "$arch" in
  arm64) triple=aarch64-apple-darwin ;;
  x86_64 | x64) arch=x86_64; triple=x86_64-apple-darwin ;;
  *) echo "Kiến trúc không hỗ trợ: $arch" >&2; exit 2 ;;
esac

# `tr`: checkout từ Windows có thể là CRLF, `"$` không khớp.
version="$(tr -d '\r' < "$repo/Cargo.toml" | sed -n 's/^version = "\(.*\)"$/\1/p' | head -1)"
[ -n "$version" ] || { echo "Không đọc được version trong Cargo.toml" >&2; exit 1; }
target_dir="${CARGO_TARGET_DIR:-$repo/target}"
exe="$target_dir/$triple/release/groove"

if [ -z "$skip_build" ]; then
  (cd "$repo" && cargo build --release --locked --target "$triple" -p groove-app --bin groove)
fi

[ -n "$out" ] || out="$repo/dist/macos-$arch"
app="$out/Groovex.app"
resources="$app/Contents/Resources"
rm -rf "$out"
mkdir -p "$app/Contents/MacOS" "$resources"
cp "$exe" "$app/Contents/MacOS/groove"
cp "$repo/packaging/macos/Info.plist" "$app/Contents/Info.plist"
plutil -replace CFBundleShortVersionString -string "$version" "$app/Contents/Info.plist"
plutil -replace CFBundleVersion -string "$version" "$app/Contents/Info.plist"
plutil -lint -s "$app/Contents/Info.plist"
cp "$repo/packaging/macos/groove.icns" "$resources/"

warnings=""
if command -v pwsh >/dev/null 2>&1 && command -v cargo-about >/dev/null 2>&1; then
  pwsh -NoProfile -File "$repo/packaging/licenses/gen-licenses.ps1" -Out "$resources/licenses"
elif [ -n "$draft" ]; then
  warnings="⚠ Không có trang giấy phép: cần pwsh + cargo-about."$'\n'
else
  echo "Không có trang giấy phép: cần pwsh + cargo-about (bản thử: --draft)." >&2
  exit 1
fi

# Ký từ trong ra ngoài, không `--deep`.
codesign --force --sign - --identifier com.sandtechs.desktop.groovex.groove "$app/Contents/MacOS/groove"
codesign --force --sign - --identifier com.sandtechs.desktop.groovex "$app"
codesign --verify --deep --strict "$app"

suffix=""
[ -z "$draft" ] || suffix="-draft"
dmg="$out/groovex-$version-$arch$suffix.dmg"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
cp -R "$app" "$stage/"
ln -s /Applications "$stage/Applications"
hdiutil create -quiet -volname "Groovex $version" -srcfolder "$stage" -format UDZO -ov "$dmg"

printf '\nXong: %s (%s)\n%s\n%s' "$dmg" "$(du -h "$dmg" | cut -f1)" "$app" "$warnings"
