// gnome-blur (09/11/2026): Groovex cho GNOME Shell — nền mờ phía sau cửa sổ chính.
//
// Cửa sổ chính mở nền `Blurred` (cột trái trong suốt). GPUI chỉ xin mờ qua `org_kde_kwin_blur` (KWin); Mutter không có giao
// thức nào cho app xin làm mờ phía sau ⇒ trên GNOME nền chỉ trong suốt trơn, chữ cửa sổ dưới lộ rõ. Extension gắn một lớp
// `Shell.BlurEffect` (BACKGROUND) làm con dưới cùng của actor cửa sổ chính, đúng khung cửa sổ (frame trừ buffer), cập nhật theo
// `size-changed`: đi theo cửa sổ khi kéo, phóng to, thu nhỏ; cửa sổ đóng thì đi cùng. Nền đứng yên không tốn gì (Shell chỉ vẽ lại
// khi có thay đổi).
//
// Cửa sổ chính = `app_id` `groove` (bản cài) hoặc `groove.dev` (bản debug) và đổi cỡ được — trình phát thu nhỏ (PiP) cỡ cố định,
// nền đục, không cần mờ.
//
// Tương phản cao (`org.gnome.desktop.a11y.interface high-contrast`) ⇒ không làm mờ. Thuộc tính D-Bus `Blur` cho app biết có mờ
// thật hay không để chọn alpha nền; đổi (bật / tắt tương phản cao) thì phát `PropertiesChanged`. App bật trước khi Shell nạp
// extension thì nghe `NameOwnerChanged` của BUS_NAME. Tên phải khớp `groove-app` `platform::backdrop::gnome`.
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Clutter from 'gi://Clutter';
import Shell from 'gi://Shell';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const BUS_NAME = 'com.sandtechs.desktop.groovex.Shell';
const OBJECT_PATH = '/com/sandtechs/desktop/groovex/Shell';
const VERSION = 1;
// clear-blur: 40 nhoè nền thành mảng gần đặc; 24 còn thấy hình khối cửa sổ phía sau, chữ của nó vẫn nhoè. Khớp alpha cột trái
// trong `groove-app` `platform::backdrop` (`SHELL_BLUR_ALPHA_*`).
const BLUR_RADIUS = 24;
const APP_IDS = ['groove', 'groove.dev'];

const IFACE = `<node>
  <interface name="com.sandtechs.desktop.groovex.Shell1">
    <property name="Version" type="u" access="read"/>
    <property name="Blur" type="b" access="read"/>
  </interface>
</node>`;

class Service {
    constructor() {
        // Cửa sổ chính đang theo → `{blur, unmanaged}`.
        this._windows = new Map();
        this._a11y = new Gio.Settings({schema_id: 'org.gnome.desktop.a11y.interface'});
        this._blur = blurAllowed(this._a11y);
        this._a11ySignal = this._a11y.connect('changed::high-contrast', () => this._sync());
        this._shown = new Map();
        this._created = global.display.connect('window-created', (_display, window) => this._watchShown(window));
        // Extension bật khi app đã mở (đăng nhập, app tự bật extension ngay sau khi mở cửa sổ): cửa sổ có thể chưa hiện, chưa
        // có `app_id` ⇒ chưa nhận thì chờ nó hiện.
        for (const window of global.display.list_all_windows()) {
            this._track(window);
            if (!this._windows.has(window))
                this._watchShown(window);
        }
        this._dbus = Gio.DBusExportedObject.wrapJSObject(IFACE, this);
        this._dbus.export(Gio.DBus.session, OBJECT_PATH);
        this._name = Gio.bus_own_name_on_connection(Gio.DBus.session, BUS_NAME, Gio.BusNameOwnerFlags.NONE, null, null);
    }

    get Version() {
        return VERSION;
    }

    get Blur() {
        return this._blur;
    }

    // `app_id` và giới hạn cỡ có thể tới sau lúc tạo ⇒ xét lúc cửa sổ hiện lần đầu.
    _watchShown(window) {
        const done = () => {
            for (const id of this._shown.get(window) ?? [])
                window.disconnect(id);
            this._shown.delete(window);
        };
        const shown = window.connect('shown', () => {
            done();
            this._track(window);
        });
        const gone = window.connect('unmanaged', done);
        this._shown.set(window, [shown, gone]);
    }

    _track(window) {
        if (!isMain(window) || this._windows.has(window))
            return;
        if (!window.get_compositor_private())
            return;
        const entry = {blur: this._blur ? blurBehind(window) : null, unmanaged: 0};
        entry.unmanaged = window.connect('unmanaged', () => {
            entry.blur?.destroy();
            this._windows.delete(window);
        });
        this._windows.set(window, entry);
    }

    _sync() {
        const blur = blurAllowed(this._a11y);
        if (blur === this._blur)
            return;
        this._blur = blur;
        for (const [window, entry] of this._windows) {
            entry.blur?.destroy();
            entry.blur = blur ? blurBehind(window) : null;
        }
        this._dbus.emit_property_changed('Blur', new GLib.Variant('b', blur));
    }

    destroy() {
        this._a11y.disconnect(this._a11ySignal);
        global.display.disconnect(this._created);
        for (const [window, ids] of this._shown)
            ids.forEach(id => window.disconnect(id));
        this._shown.clear();
        for (const [window, entry] of this._windows) {
            window.disconnect(entry.unmanaged);
            entry.blur?.destroy();
        }
        this._windows.clear();
        Gio.bus_unown_name(this._name);
        this._dbus.unexport();
    }
}

function blurAllowed(a11y) {
    return !a11y.get_boolean('high-contrast');
}

function isMain(window) {
    return APP_IDS.includes(window.get_wm_class()) && window.allows_resize();
}

// Lớp mờ dưới cùng trong actor cửa sổ, khớp khung cửa sổ (actor gồm cả phần bóng / viền vô hình nếu có). Trả `{destroy()}`;
// actor cửa sổ mất trước (cửa sổ đóng) thì lớp mờ đi cùng.
function blurBehind(window) {
    const actor = window.get_compositor_private();
    if (!actor)
        return null;
    const layer = new Clutter.Actor({reactive: false});
    layer.add_effect(new Shell.BlurEffect({mode: Shell.BlurMode.BACKGROUND, radius: BLUR_RADIUS, brightness: 1.0}));
    actor.insert_child_below(layer, null);
    const fit = () => {
        const frame = window.get_frame_rect();
        const buffer = window.get_buffer_rect();
        layer.set_position(frame.x - buffer.x, frame.y - buffer.y);
        layer.set_size(frame.width, frame.height);
    };
    fit();
    const resized = window.connect('size-changed', fit);
    let gone = false;
    layer.connect('destroy', () => {
        gone = true;
    });
    return {
        destroy() {
            window.disconnect(resized);
            if (!gone)
                layer.destroy();
        },
    };
}

export default class GroovexShell extends Extension {
    enable() {
        this._service = new Service();
    }

    disable() {
        this._service?.destroy();
        this._service = null;
    }
}
