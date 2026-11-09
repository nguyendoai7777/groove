#!/usr/bin/env bash
# Mở Groovex bản debug để xem tay: dữ liệu thật của máy, cửa sổ ở nơi OS hoặc vị trí đã lưu chọn. Gốc workspace gọi sang đây
# (`bash start.sh groove`, `pwsh start.ps1 groove`); chạy thẳng cũng được, kể cả từ worktree (build ở target/w<N>):
#   bash groove/start.sh [--no-watch]
# Mặc định theo dõi mã (tools/watch-run.mjs của gốc): lưu tệp ⇒ build lại, mở lại app (mất trạng thái đang mở).
# --no-watch hoặc XDESKTOP_NO_WATCH=1: build rồi chạy một lần. Bản debug mã riêng `groove.dev` (MPRIS `groovex.dev`, Windows
# AUMID `GrooveX.Player.Dev`): chạy cạnh bản đã cài không đè nhau. Linux: chạy trong scope systemd riêng
# `app-xdesktop-groove.dev-*.scope` (Mission Center hiện dòng riêng), ghi mục "Groovex (dev)" cho ô tìm + dock.
#   bash groove/start.sh --plans   groove chưa có plan ⇒ in rỗng (start.sh gốc bỏ bước hỏi plan)
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$repo"
until [ -f "$root/tools/script-lib.sh" ]; do
  [ "$root" != / ] || { echo "Không thấy gốc workspace xdesktop (tools/script-lib.sh) phía trên $repo" >&2; exit 1; }
  root="$(dirname "$root")"
done
. "$root/tools/script-lib.sh"
repo="$(xd_path "$repo")"

xd_parse_watch "$@"
for a in "${xd_args[@]+"${xd_args[@]}"}"; do
  case "$a" in
    --plans) exit 0 ;;
    -h|--help|help) sed -n '2,/^set -euo/p' "$0" | grep '^#' | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "Không hiểu tham số: $a" >&2; exit 1 ;;
  esac
done

# Wayland không cho app tự gửi icon: GNOME ghép cửa sổ theo `app_id` với `<app_id>.desktop` để lấy icon cho dock.
# Bản debug có mã riêng `groove.dev` (groove `window_state::DEV_APP_ID`, MPRIS `groovex.dev`) ⇒ mục `groove.dev.desktop`
# "Groovex (dev)" hiện trong ô tìm, cạnh "Groovex" của gói đã cài, không đè nhau. Không MimeType: không giành việc mở tệp nhạc.
# Mission Center bản snap ghép scope `app-xdesktop-groove.dev-*` với mục đó, đọc home riêng của snap trước
# (XDG_DATA_HOME = ~/snap/mission-center/current/.local/share). Snap không có plug `home` ⇒ không đọc được ảnh trong repo:
# ghi thêm một bản vào home snap, icon trỏ tới ảnh chép sang đó. Không `NoDisplay` / `Hidden`: app-rummage bỏ hẳn các mục đó.
groove_dev_desktop_entry() {
  local icon="$repo/packaging/linux/icons/256x256.png" mc="$HOME/snap/mission-center/current/.local/share"
  groove_dev_desktop_entry_in "${XDG_DATA_HOME:-$HOME/.local/share}/applications" "$icon"
  if [ -d "$HOME/snap/mission-center/current" ]; then
    mkdir -p "$mc/icons" && cp -u "$icon" "$mc/icons/groove-dev.png"
    groove_dev_desktop_entry_in "$mc/applications" "$mc/icons/groove-dev.png"
  fi
}

groove_dev_desktop_entry_in() {
  local dir="$1" icon="$2"
  local file="$dir/groove.dev.desktop" old="$dir/groove.desktop" marker="X-Xdesktop-Dev=true"
  # Bản cũ ghi mục dev trùng tên `groove.desktop` của gói (che mục của gói): gỡ nếu đúng là của script.
  if [ -f "$old" ] && grep -qx "$marker" "$old"; then rm -f "$old"; fi
  if [ -f "$file" ] && ! grep -qx "$marker" "$file"; then return 0; fi
  mkdir -p "$dir"
  cat > "$file.tmp" <<ENTRY
[Desktop Entry]
Type=Application
Name=Groovex (dev)
Exec=$CARGO_TARGET_DIR/debug/groove
Icon=$icon
Terminal=false
Categories=AudioVideo;Audio;Player;
StartupWMClass=groove.dev
$marker
Actions=pip;

[Desktop Action pip]
Name=Picture in picture
Name[vi]=Trình phát thu nhỏ
Exec=$CARGO_TARGET_DIR/debug/groove --pip
ENTRY
  mv "$file.tmp" "$file"
}

# gnome-blur: GNOME không làm mờ sau cửa sổ cho app — extension Shell của Groovex làm (`platform::backdrop::gnome`). Bản debug
# không qua gói `.deb` ⇒ nối extension của master vào thư mục extension người dùng (bản này đè bản của gói, luôn mới theo
# master). Shell chỉ quét thư mục này lúc đăng nhập: nối lần đầu thì đăng xuất / đăng nhập lại, app tự bật extension.
groove_dev_shell_extension() {
  local uuid="groovex-shell@com.sandtechs.desktop.groovex"
  local dir="${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions"
  case ":${XDG_CURRENT_DESKTOP:-}:" in *:GNOME:*) ;; *) return 0 ;; esac
  [ -e "$dir/$uuid" ] && return 0
  mkdir -p "$dir" && ln -s "$root/groove/packaging/linux/gnome-shell/$uuid" "$dir/$uuid" \
    && echo "Đã nối extension GNOME Shell của Groovex (nền mờ): đăng xuất rồi đăng nhập lại để Shell nạp."
}

# Master build ở groove/target/start; worktree w<N> build ở groove/target/w<N>.
export CARGO_TARGET_DIR="$root/groove/target/$(xd_target_name "$repo" groove)"
echo "groove từ $repo"
[ "$(uname -s)" = Linux ] && { groove_dev_desktop_entry; groove_dev_shell_extension; }
m="$root/materiant"
xd_run_app --scope groove.dev "$CARGO_TARGET_DIR/debug/groove" \
  "$repo/crates|$repo/i18n|$repo/Cargo.toml|$m/src|$m/assets|$m/vendor|$m/Cargo.toml" \
  -- --manifest-path "$repo/Cargo.toml" -p groove-app
