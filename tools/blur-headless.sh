#!/usr/bin/env bash
# gnome-blur (09/11/2026): Linux GNOME — nền mờ sau cửa sổ chính (extension `packaging/linux/gnome-shell/groovex-shell@…`).
# GNOME Shell headless (bus riêng qua dbus-run-session, không đụng phiên GNOME đang dùng) + extension thật của repo + extension
# dò vẽ nền chữ sắc dưới mọi cửa sổ, chụp stage. Hai lượt:
#   live — extension cài mà CHƯA bật, app mở trước: app tự bật nó (`EnableExtension`), nghe `NameOwnerChanged` rồi vẽ lại ⇒
#          có lớp `Shell.BlurEffect` khớp khung cửa sổ, `Blur` = true (ảnh `blur-on.png`); rồi bật tương phản cao lúc đang
#          chạy ⇒ gỡ lớp mờ, `Blur` = false, app vẽ lại nền đục (ảnh `high-contrast-live.png`).
#   hc   — tương phản cao bật từ đầu ⇒ không lớp mờ, `Blur` = false (ảnh `high-contrast.png`).
#   max  — extension bật từ đầu (như sau đăng nhập), app mở lại ở trạng thái phóng to đã lưu ⇒ vẫn có lớp mờ (ảnh
#          `maximized.png`; bản 1 của extension bỏ sót: Mutter trả `allows_resize()` = false cho cửa sổ phóng to). Rồi mở
#          trình phát thu nhỏ (`groove --pip`) ⇒ app gọi `KeepAbove`, extension ghim đúng cửa sổ PiP lên trên, cửa sổ chính không
#          (pip-above, bản 3; ảnh `pip.png`).
#   cargo build -p groove-app && bash tools/blur-headless.sh [<target>/debug]
# Dữ liệu, ảnh: `<target>/blur-headless/` (xoá đầu lượt, giữ sau lượt để xem).
set -u
if [ -z "${BLUR_NESTED:-}" ]; then
  repo="$(cd "$(dirname "$0")/.." && pwd)"
  bin="$(realpath "${1:-${CARGO_TARGET_DIR:-$repo/target}/debug}")"
  out="$(dirname "$bin")/blur-headless"
  rm -rf "$out" && mkdir -p "$out"
  fail=0
  for run in live hc max; do
    BLUR_NESTED=1 dbus-run-session -- bash "$0" "$bin" "$out" "$run" || fail=1
  done
  echo "ảnh: $out"
  exit $fail
fi
repo="$(cd "$(dirname "$0")/.." && pwd)"
bin=$1; out=$2; run=$3
work="$out/$run"
uuid="groovex-shell@com.sandtechs.desktop.groovex"
export XDG_CONFIG_HOME="$work/config" XDG_DATA_HOME="$work/data" XDG_STATE_HOME="$work/state" XDG_CACHE_HOME="$work/cache"
export BLUR_PROBE_DIR="$work"
exts="$XDG_DATA_HOME/gnome-shell/extensions"
mkdir -p "$exts/blur-probe@groove" "$XDG_CONFIG_HOME/glib-2.0/settings"
cp -r "$repo/packaging/linux/gnome-shell/$uuid" "$exts/"
hc=false; enabled="'blur-probe@groove'"
[ "$run" = hc ] && { hc=true; enabled="'$uuid', 'blur-probe@groove'"; }
[ "$run" = max ] && enabled="'$uuid', 'blur-probe@groove'"
printf "[org/gnome/shell]\nenabled-extensions=[%s]\ndisable-user-extensions=false\n[org/gnome/desktop/a11y/interface]\nhigh-contrast=%s\n[org/gnome/desktop/interface]\ncolor-scheme='prefer-dark'\n" "$enabled" "$hc" \
  > "$XDG_CONFIG_HOME/glib-2.0/settings/keyfile"
echo '{"uuid":"blur-probe@groove","name":"blur-probe","description":"groove test","shell-version":["45","46","47","48","49","50"]}' \
  > "$exts/blur-probe@groove/metadata.json"
cat > "$exts/blur-probe@groove/extension.js" <<'JS'
import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import St from 'gi://St';
import Shell from 'gi://Shell';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
Gio._promisify(Shell.Screenshot.prototype, 'screenshot', 'screenshot_finish');
// Nền chữ sắc ngay trên hình nền (dưới mọi cửa sổ). Có tệp `shot` (nội dung = tên ảnh) ⇒ ghi con của actor cửa sổ Groovex
// vào `actors.txt` và chụp stage ra `<tên>.png`.
export default class Probe extends Extension {
    enable() {
        const dir = GLib.getenv('BLUR_PROBE_DIR');
        const W = global.stage.width, H = global.stage.height;
        this._bg = new St.BoxLayout({vertical: true, style: 'background-color: #f4f4f4;', width: W, height: H});
        const line = 'const launcher = window.open(url); // ~/.local/share 0123456789 ABCDEFGH ';
        for (let i = 0; i < Math.ceil(H / 22); i++)
            this._bg.add_child(new St.Label({text: line.repeat(4), style: `color: ${i % 3 ? '#222' : '#c0392b'}; font-size: 15px;`}));
        global.window_group.insert_child_above(this._bg, Main.layoutManager._backgroundGroup);
        this._poll = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 200, () => {
            const cmd = Gio.File.new_for_path(`${dir}/shot`);
            if (!cmd.query_exists(null))
                return GLib.SOURCE_CONTINUE;
            const name = new TextDecoder().decode(cmd.load_contents(null)[1]).trim();
            cmd.delete(null);
            const lines = global.get_window_actors().filter(a => a.meta_window.get_wm_class()?.startsWith('groove')).map(a => {
                const f = a.meta_window.get_frame_rect();
                const kids = a.get_children().map(c => `${c.get_effects().some(e => e instanceof Shell.BlurEffect) ? 'blur' : 'surface'} ${c.x},${c.y} ${c.width}x${c.height}`);
                return `${a.meta_window.get_wm_class()} above=${a.meta_window.is_above()} frame ${f.width}x${f.height}: ${kids.join(' | ')}`;
            });
            GLib.file_set_contents(`${dir}/actors.txt`, `${lines.join('\n')}\n`);
            const stream = Gio.File.new_for_path(`${dir}/${name}.png`).replace(null, false, Gio.FileCreateFlags.NONE, null);
            new Shell.Screenshot().screenshot(false, stream).then(() => {
                stream.close(null);
                GLib.file_set_contents(`${dir}/shot-done`, '');
            });
            return GLib.SOURCE_CONTINUE;
        });
    }

    disable() {
        GLib.source_remove(this._poll);
        this._bg.destroy();
    }
}
JS
export GSETTINGS_BACKEND=keyfile
disp="groove-blur-$$"
gnome-shell --headless --wayland-display="$disp" --no-x11 --virtual-monitor 1600x900 > "$work/shell.log" 2>&1 &
shell=$!
app=""
trap 'kill $app $shell 2>/dev/null; wait 2>/dev/null' EXIT
for _ in $(seq 80); do
  gdbus call --session -d org.gnome.Shell -o /org/gnome/Shell -m org.gnome.Shell.Extensions.GetExtensionInfo "$uuid" \
    2>/dev/null | grep -q "'uuid'" && break
  sleep 0.25
done

blur_prop() {
  gdbus call --session -d com.sandtechs.desktop.groovex.Shell -o /com/sandtechs/desktop/groovex/Shell \
    -m org.freedesktop.DBus.Properties.Get com.sandtechs.desktop.groovex.Shell1 Blur 2>/dev/null || echo "(không chạy)"
}

shot() {
  rm -f "$work/shot-done" "$work/actors.txt"
  echo "$1" > "$work/shot"
  for _ in $(seq 40); do [ -e "$work/shot-done" ] && break; sleep 0.25; done
  cat "$work/actors.txt" 2>/dev/null
}

# want = true | false: `Blur` và lớp mờ khớp khung dưới mặt cửa sổ.
check() {
  local want=$1 props=$2 actors=$3 ok=1
  echo "  Blur=$props"
  echo "  $actors"
  grep -q "<$want>" <<< "$props" || { echo "  SAI: Blur phải là $want"; ok=0; }
  if [ "$want" = true ]; then
    grep -qE "frame ([0-9]+x[0-9]+): blur 0,0 \1 \| surface" <<< "$actors" || { echo "  SAI: thiếu lớp mờ khớp khung dưới cửa sổ"; ok=0; }
  else
    grep -q "blur" <<< "$actors" && { echo "  SAI: không được có lớp mờ"; ok=0; }
  fi
  [ -n "$actors" ] || { echo "  SAI: không thấy cửa sổ Groovex"; ok=0; }
  [ $ok = 1 ] && echo "  ok"
  [ $ok = 1 ]
}

export WAYLAND_DISPLAY="$disp" XDG_CURRENT_DESKTOP=GNOME GROOVE_DATA_DIR="$work/groove" GROOVE_LOG=info
unset DISPLAY
mkdir -p "$GROOVE_DATA_DIR"
if [ "$run" = max ]; then
  echo '{"theme":"dark","window":{"x":0,"y":0,"width":900,"height":700,"maximized":true}}' > "$GROOVE_DATA_DIR/settings.json"
else
  echo '{"theme":"dark"}' > "$GROOVE_DATA_DIR/settings.json"
fi
"$bin/groove" > "$work/app.log" 2>&1 &
app=$!
sleep 8
ok=1
if [ "$run" = live ]; then
  echo "lượt live: extension chưa bật, app mở trước"
  check true "$(blur_prop)" "$(shot blur-on)" || ok=0
  grep -q "GNOME Shell blur on" "$work/app.log" || { echo "  SAI: app không thấy Blur bật (app.log)"; ok=0; }
  echo "lượt live: bật tương phản cao lúc đang chạy"
  gsettings set org.gnome.desktop.a11y.interface high-contrast true
  sleep 3
  check false "$(blur_prop)" "$(shot high-contrast-live)" || ok=0
  grep -q "GNOME Shell blur off" "$work/app.log" || { echo "  SAI: app không thấy Blur tắt (app.log)"; ok=0; }
elif [ "$run" = max ]; then
  echo "lượt max: extension bật từ đầu, cửa sổ mở sẵn phóng to"
  check true "$(blur_prop)" "$(shot maximized)" || ok=0
  echo "lượt max: mở trình phát thu nhỏ"
  "$bin/groove" --pip > "$work/pip.log" 2>&1
  sleep 3
  actors="$(shot pip)"
  echo "  $actors"
  [ "$(grep -c 'above=true' <<< "$actors")" = 1 ] && [ "$(grep -c 'above=false' <<< "$actors")" = 1 ] \
    && echo "  ok" || { echo "  SAI: phải có đúng PiP ghim trên cùng, cửa sổ chính không"; ok=0; }
  grep -q "GNOME Shell keeps the window above" "$work/app.log" || { echo "  SAI: app không gọi được KeepAbove (app.log)"; ok=0; }
else
  echo "lượt hc: tương phản cao từ đầu"
  check false "$(blur_prop)" "$(shot high-contrast)" || ok=0
fi
mv "$work"/*.png "$out/" 2>/dev/null
[ $ok = 1 ]
