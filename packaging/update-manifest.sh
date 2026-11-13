#!/usr/bin/env bash
# linux-blur-update (13/11/2026): manifest cập nhật cạnh bộ cài — `groovex-latest-<khoá>.json` (`groove-app` `updater`).
#   bash packaging/update-manifest.sh <bộ cài> <khoá nền> <version>
# Khoá: `linux-amd64`, `macos-arm64`, `macos-x64` (`updater::platform_key`; Windows ghi trong `build-installer.ps1`). Trang tải
# đặt bộ cài với tên `groovex-latest-<khoá>.<đuôi>` cạnh tệp json này ở https://groovex.sandtechs.com/download/; app tính
# địa chỉ bộ cài từ `file`, soát `size` + `sha256` sau khi tải.
set -euo pipefail
installer=$1 key=$2 version=$3
ext="${installer##*.}"
if command -v sha256sum >/dev/null 2>&1; then sha="$(sha256sum "$installer" | cut -d' ' -f1)"; else sha="$(shasum -a 256 "$installer" | cut -d' ' -f1)"; fi
size="$(wc -c < "$installer" | tr -d ' ')"
json="$(dirname "$installer")/groovex-latest-$key.json"
printf '{"version":"%s","file":"groovex-latest-%s.%s","size":%s,"sha256":"%s"}\n' "$version" "$key" "$ext" "$size" "$sha" > "$json"
echo "Manifest cập nhật: $json (đăng kèm bộ cài với tên groovex-latest-$key.$ext)"
