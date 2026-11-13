#!/usr/bin/env bash
# Dựng `<out>/Groovex.app` + `groovex_<version>_macos_<arm64|x64>[_draft].dmg`, ký ad-hoc (groove-gd1_27). Chạy trên Mac. Không dùng Developer ID: đó là
# dịch vụ trả phí, chủ dự án quyết; lần đầu mở thì Gatekeeper cảnh báo, chuột phải > Open.
#   bash packaging/macos/build-macos.sh                # bản phát hành: thiếu trang giấy phép là dừng
#   bash packaging/macos/build-macos.sh --draft        # bản thử: thiếu thì cảnh báo, tên `.dmg` có `_draft`
# Thêm: --arch arm64|x86_64|x64 (mặc định kiến trúc máy; Apple Silicon ⇄ Intel cross-compile được, cần `rustup target add
# <triple>`), --skip-build, --out <thư mục> (mặc định dist/macos-<arch>).
# Cần: `cargo install cargo-about --locked --features cli`; trang giấy phép qua `gen-licenses.ps1` khi có `pwsh`, không thì bản
# Node `gen-licenses.mjs` (cùng bước, cùng chốt soát — như build-deb.sh).
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
# Chỉ xoá `.app` cũ: `--out` có thể là thư mục chung chứa bản dựng khác (release/groove).
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$resources"
cp "$exe" "$app/Contents/MacOS/groove"
cp "$repo/packaging/macos/Info.plist" "$app/Contents/Info.plist"
plutil -replace CFBundleShortVersionString -string "$version" "$app/Contents/Info.plist"
plutil -replace CFBundleVersion -string "$version" "$app/Contents/Info.plist"
plutil -lint -s "$app/Contents/Info.plist"
cp "$repo/packaging/macos/groove.icns" "$resources/"

warnings=""
if command -v cargo-about >/dev/null 2>&1 && command -v pwsh >/dev/null 2>&1; then
  pwsh -NoProfile -File "$repo/packaging/licenses/gen-licenses.ps1" -Out "$resources/licenses"
elif command -v cargo-about >/dev/null 2>&1 && command -v node >/dev/null 2>&1; then
  node "$repo/packaging/licenses/gen-licenses.mjs" "$resources/licenses"
elif [ -n "$draft" ]; then
  warnings="⚠ Không có trang giấy phép: cần cargo-about + pwsh hoặc Node."$'\n'
else
  echo "Không có trang giấy phép: cần cargo-about + pwsh hoặc Node (bản thử: --draft)." >&2
  exit 1
fi

# Ký từ trong ra ngoài, không `--deep`.
codesign --force --sign - --identifier com.sandtechs.desktop.groovex.groove "$app/Contents/MacOS/groove"
codesign --force --sign - --identifier com.sandtechs.desktop.groovex "$app"
codesign --verify --deep --strict "$app"

suffix=""
[ -z "$draft" ] || suffix="_draft"
# Tên tệp: {app}_{version}_{os}_{arch}[_draft].dmg, x86_64 viết là x64.
dmg="$out/groovex_${version}_macos_${arch/x86_64/x64}$suffix.dmg"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
cp -R "$app" "$stage/"
ln -s /Applications "$stage/Applications"
hdiutil create -quiet -volname "Groovex $version" -srcfolder "$stage" -format UDZO -ov "$dmg"
# Tệp cập nhật (groove-gd2_05): `.app.zip` (`ditto` giữ quyền, liên kết, chữ ký trong bundle; bỏ xattr ⇒ không sinh `._*`) —
# app tải qua server `cloud`, soát sha256 rồi thay `Groovex.app` (crate `updater` của materiant).
zip="${dmg%.dmg}.app.zip"
rm -f "$zip"
ditto -c -k --norsrc --noextattr --noqtn --keepParent "$app" "$zip"
for f in "$dmg" "$zip"; do (cd "$(dirname "$f")" && shasum -a 256 "$(basename "$f")" > "$(basename "$f").sha256"); done

printf '\nXong: %s (%s)\n%s (%s, cập nhật)\n%s\n%s' "$dmg" "$(du -h "$dmg" | cut -f1)" "$zip" "$(du -h "$zip" | cut -f1)" "$app" "$warnings"
# Lệnh đưa bản lên kênh cập nhật (server `cloud`): không đưa bản `_draft`.
if [ -z "$draft" ]; then
  printf '\nĐưa lên kênh cập nhật của staging (đứng ở gốc xdesktop):\n  bash cloud/deploy/publish-release.sh groove %s --version %s --os macos --arch %s --plan free --notes groove/release-notes/v%s.md\n' \
    "$zip" "$version" "${arch/x86_64/x64}" "$version"
fi
