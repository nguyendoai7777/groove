#!/usr/bin/env bash
# Build bản cài Groovex cho máy đang chạy, ra <gốc>/release/groove/. Gốc workspace gọi sang đây
# (`bash build.sh --project groove`); chạy thẳng cũng được:
#   bash groove/build.sh [-f | --force]    # --force: biên dịch lại mã của ta, giữ thư viện đã build
#   bash groove/build.sh --plans           # groove chưa có plan ⇒ in rỗng
# Dựng thật: packaging/linux/build-deb.sh (.deb), packaging/macos/build-macos.sh (.app + .dmg); Windows (Git Bash) ⇒ build.ps1
# (bản cài NSIS, packaging/windows/build-installer.ps1). Chỉ build cho máy đang chạy: không nhận --os, --arch, --chip.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$repo"
until [ -f "$root/tools/script-lib.sh" ]; do
  [ "$root" != / ] || { echo "Không thấy gốc workspace xdesktop (tools/script-lib.sh) phía trên $repo" >&2; exit 1; }
  root="$(dirname "$root")"
done
. "$root/tools/script-lib.sh"
repo="$(xd_path "$repo")"

force=""
while [ $# -gt 0 ]; do
  case "$1" in
    --plans) exit 0 ;;
    -f|--force) force=1; shift ;;
    --os|--arch|--chip) echo "groove chỉ build cho máy đang chạy, không nhận --os, --arch, --chip." >&2; exit 1 ;;
    *) echo "Không hiểu tham số: $1" >&2; exit 1 ;;
  esac
done

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) exec pwsh -NoProfile -File "$(cygpath -w "$repo/build.ps1")" ${force:+-Force} ;;
  Darwin) host_os=macos ;;
  *) host_os=linux ;;
esac
if [ -n "$force" ]; then
  clean_target=()
  if [ "$host_os" = macos ]; then
    case "$(uname -m)" in arm64|aarch64) clean_target=(--target aarch64-apple-darwin) ;; *) clean_target=(--target x86_64-apple-darwin) ;; esac
  fi
  cargo clean --release --manifest-path "$repo/Cargo.toml" ${clean_target[@]+"${clean_target[@]}"} \
    -p groove-app -p groove -p groove-audio -p groove-core -p groove-library -p materiant
fi
case "$host_os" in
  macos) exec bash "$repo/packaging/macos/build-macos.sh" --out "$root/release/groove" ;;
  *) exec bash "$repo/packaging/linux/build-deb.sh" --out "$root/release/groove" ;;
esac
