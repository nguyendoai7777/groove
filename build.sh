#!/usr/bin/env bash
# Build bản cài Groovex, ra <gốc>/release/groove/. Gốc workspace gọi sang đây (`bash build.sh --project groove`); chạy thẳng
# cũng được:
#   bash groove/build.sh [-f | --force]                     # --force: biên dịch lại mã của ta, giữ thư viện đã build
#   bash groove/build.sh --arch x64                         # macOS: Intel (x86_64-apple-darwin), build được trên Apple Silicon
#   bash groove/build.sh --os macos --arch arm64 --chip apple
#   bash groove/build.sh --plans                            # groove chưa có plan ⇒ in rỗng
# macOS: hỏi kiến trúc (Enter = máy đang chạy; không tty ⇒ máy đang chạy), Apple Silicon ⇄ Intel cross-compile được, luôn build
# với --target; --chip apple|intel tương đương --arch arm64|x64. Ra Groovex.app + groovex_<version>_macos_<arm64|x64>.dmg (hai
# kiến trúc dùng chung tên .app: dựng cái sau đè .app cái trước, .dmg giữ cả hai).
# Windows (Git Bash ⇒ build.ps1, bản cài NSIS) và Linux (.deb) chỉ build cho máy đang chạy: --os / --arch khác máy thì dừng.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$repo"
until [ -f "$root/tools/script-lib.sh" ]; do
  [ "$root" != / ] || { echo "Không thấy gốc workspace xdesktop (tools/script-lib.sh) phía trên $repo" >&2; exit 1; }
  root="$(dirname "$root")"
done
. "$root/tools/script-lib.sh"
repo="$(xd_path "$repo")"

force="" os="" arch="" chip=""
while [ $# -gt 0 ]; do
  case "$1" in
    --plans) exit 0 ;;
    -f|--force) force=1; shift ;;
    --os|--arch|--chip)
      [ $# -ge 2 ] || { echo "$1 cần giá trị." >&2; exit 1; }
      case "$1" in --os) os="$2" ;; --arch) arch="$2" ;; --chip) chip="$2" ;; esac
      shift 2 ;;
    --os=*) os="${1#--os=}"; shift ;;
    --arch=*) arch="${1#--arch=}"; shift ;;
    --chip=*) chip="${1#--chip=}"; shift ;;
    *) echo "Không hiểu tham số: $1" >&2; exit 1 ;;
  esac
done

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) host_os=windows ;;
  Darwin) host_os=macos ;;
  *) host_os=linux ;;
esac
case "$(uname -m)" in arm64|aarch64) host_arch=arm64 ;; *) host_arch=x64 ;; esac
case "$arch" in aarch64) arch=arm64 ;; x86_64|amd64) arch=x64 ;; esac
case "$os" in ""|"$host_os") ;; *) echo "groove chỉ build được cho OS đang chạy ($host_os), không phải $os." >&2; exit 1 ;; esac

if [ "$host_os" = macos ]; then
  case "$chip" in
    "") ;;
    apple) [ -z "$arch" ] || [ "$arch" = arm64 ] || { echo "Chip apple đi với --arch arm64." >&2; exit 1; }; arch=arm64 ;;
    intel) [ -z "$arch" ] || [ "$arch" = x64 ] || { echo "Chip intel đi với --arch x64." >&2; exit 1; }; arch=x64 ;;
    *) echo "--chip ở macOS: apple hoặc intel." >&2; exit 1 ;;
  esac
  if [ -z "$arch" ]; then
    case "$host_arch" in arm64) d=1 ;; *) d=2 ;; esac
    xd_select 'Kiến trúc:' "$d" 'arm64|arm64 — Apple Silicon (M1 trở lên)' 'x64|x64 — Intel'
    arch="$picked"
  fi
  case "$arch" in
    arm64) triple=aarch64-apple-darwin ;;
    x64) triple=x86_64-apple-darwin ;;
    *) echo "--arch ở macOS: arm64 hoặc x64." >&2; exit 1 ;;
  esac
  if command -v rustup >/dev/null 2>&1 && ! rustup target list --installed | grep -qx "$triple"; then
    echo "==> rustup target add $triple"
    rustup target add "$triple"
  fi
else
  [ -z "$chip" ] || { echo "groove chỉ nhận --chip ở macOS." >&2; exit 1; }
  case "$arch" in ""|"$host_arch") ;; *) echo "groove ở $host_os chỉ build cho kiến trúc máy đang chạy ($host_arch)." >&2; exit 1 ;; esac
fi

[ "$host_os" != windows ] || exec pwsh -NoProfile -File "$(cygpath -w "$repo/build.ps1")" ${force:+-Force}
if [ -n "$force" ]; then
  clean_target=()
  [ "$host_os" != macos ] || clean_target=(--target "$triple")
  cargo clean --release --manifest-path "$repo/Cargo.toml" ${clean_target[@]+"${clean_target[@]}"} \
    -p groove-app -p groove -p groove-audio -p groove-core -p groove-library -p materiant
fi
case "$host_os" in
  macos) printf '\n==> %s (%s)\n' "$triple" "$arch"
    exec bash "$repo/packaging/macos/build-macos.sh" --arch "$arch" --out "$root/release/groove" ;;
  *) exec bash "$repo/packaging/linux/build-deb.sh" --out "$root/release/groove" ;;
esac
