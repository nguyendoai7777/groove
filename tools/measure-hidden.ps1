# Chạy groove trên một Win32 desktop ẩn: cửa sổ thật được tạo, nhưng không hiện ở màn nào, không giành focus. Dùng khi máy chỉ một
# màn, `tools/run.ps1` từ chối. Dữ liệu ở thư mục tạm, xoá sau khi xong; exe phải là bản có debug assertions, không thì
# `GROOVE_DATA_DIR` bị bỏ qua và app mở thư viện thật, nên script soát trước và từ chối.
#
#   cargo build -p groove-app                                             # bản debug
#   pwsh tools/measure-hidden.ps1 -Exe target/debug/groove.exe
#
#   # Gần với bản phát hành: tối ưu như release, chỉ thêm debug assertions để biến môi trường còn hiệu lực.
#   $env:CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS = "true"; cargo build --release -p groove-app
#   pwsh tools/measure-hidden.ps1 -Exe target/release/groove.exe -Keys "g p wait@500 ctrl-k wait@500"
#
#   # Dữ liệu tạm có sẵn một bài tên dài đang mở (không phát) để thanh phát không khoá, tên chạy ngang:
#   cargo build -p groove-library --example seed_data
#   pwsh tools/measure-hidden.ps1 -Exe target/debug/groove.exe -Seed target/debug/examples/seed_data.exe
#
# In: số cửa sổ, RAM (working set, private) và CPU trung bình từ mốc trước (% của một nhân) ở mỗi mốc, dòng log WARN / ERROR,
# `foreground_unchanged`. `-Seed <exe>` chạy `<exe> <thư mục dữ liệu tạm> [-Songs]` trước khi mở app; `-Songs <n>` thêm n bài vào
# thư viện tạm (đo với thư viện có bài).
#
# `-Clicks "max,max,min,close"`: bấm nút cửa sổ ở góc trên phải bằng thông điệp Win32 như khi rê + nhấp chuột thật
# (`WM_MOUSEMOVE` → `WM_NCHITTEST` → `WM_NC*` hoặc `WM_LBUTTON*` theo kết quả hit-test), rồi in hit-test và trạng thái
# thật `IsIconic` / `IsZoomed` sau mỗi lần bấm. Sau `min` script tự `SW_RESTORE` để bấm tiếp. Bước `hit@x:y` chỉ rê tới điểm
# (px logic, x / y âm tính từ mép phải / dưới) và in hit-test; `dbl@x:y` nhấp đúp ở đó (vùng kéo: phóng to / khôi phục). Kéo chuột:
# `down@x:y`, `move@x:y`, `up@x:y` (nhấn / rê / thả, đi đường WM_NC* nếu hit-test không phải HTCLIENT như chuột thật);
# `shot@<tệp.png>` chụp cửa sổ giữa các bước; `nudge@n` dời cửa sổ n lần 1 px mỗi 16 ms giữa
# `WM_ENTERSIZEMOVE` / `WM_EXITSIZEMOVE` (như kéo) và in CPU lúc dời; `settings` in `settings.json` của dữ liệu tạm; `rects` in cỡ khung và vùng client.
#
# `-Pip`: chạy kèm `--pip` và bấm / chụp cửa sổ trình phát thu nhỏ thay cửa sổ chính.
param(
    [Parameter(Mandatory)][string]$Exe,
    [int]$Seconds = 15,
    [string]$Keys = "",
    [string]$Shot = "",
    [string]$Seed = "",
    [int]$Songs = 0,
    # Mở cửa sổ ở trạng thái active (bỏ `--background`): cửa sổ vẫn nằm trên desktop ẩn, không giành focus của desktop đang dùng.
    [switch]$Active,
    [string]$Clicks = "",
    # Rê trên nút bao lâu (ms) trước khi nhấp — đủ lâu thì tooltip đã hiện.
    [int]$Hover = 150,
    # Ghi `settings.json` của dữ liệu tạm trước khi mở app, vd '{"theme":"light"}'.
    [string]$Settings = "",
    [switch]$Pip
)
$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class HiddenDesktop {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct STARTUPINFO {
        public int cb; public string lpReserved; public string lpDesktop; public string lpTitle;
        public int dwX, dwY, dwXSize, dwYSize, dwXCountChars, dwYCountChars, dwFillAttribute, dwFlags;
        public short wShowWindow, cbReserved2; public IntPtr lpReserved2, hStdInput, hStdOutput, hStdError;
    }
    [StructLayout(LayoutKind.Sequential)]
    public struct PROCESS_INFORMATION { public IntPtr hProcess, hThread; public int dwProcessId, dwThreadId; }
    public delegate bool EnumProc(IntPtr hwnd, IntPtr lparam);
    [DllImport("user32", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateDesktopW(string name, IntPtr device, IntPtr mode, int flags, uint access, IntPtr attrs);
    [DllImport("user32", SetLastError = true)] public static extern bool CloseDesktop(IntPtr desktop);
    [DllImport("user32")] public static extern bool EnumDesktopWindows(IntPtr desktop, EnumProc proc, IntPtr lparam);
    [DllImport("user32")] public static extern int GetWindowThreadProcessId(IntPtr hwnd, out int pid);
    [DllImport("user32")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr hwnd, StringBuilder text, int size);
    [DllImport("user32")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);
    [DllImport("user32")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32")] public static extern bool IsIconic(IntPtr hwnd);
    [DllImport("user32")] public static extern bool IsZoomed(IntPtr hwnd);
    [DllImport("user32")] public static extern bool IsWindow(IntPtr hwnd);
    [DllImport("user32")] public static extern bool ShowWindowAsync(IntPtr hwnd, int cmd);
    [DllImport("user32")] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32")] public static extern bool GetClientRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32")] public static extern bool ClientToScreen(IntPtr hwnd, ref POINT point);
    [DllImport("user32")] public static extern uint GetDpiForWindow(IntPtr hwnd);
    [DllImport("user32")] public static extern bool PostMessageW(IntPtr hwnd, uint msg, IntPtr wparam, IntPtr lparam);
    [DllImport("user32")] public static extern IntPtr SendMessageTimeoutW(IntPtr hwnd, uint msg, IntPtr wparam, IntPtr lparam,
        uint flags, uint timeout, out IntPtr result);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    static IntPtr Pack(int x, int y) { return (IntPtr)(((y & 0xffff) << 16) | (x & 0xffff)); }

    // Rê tới điểm (x, y) trong vùng client rồi nhấp trái như chuột thật: GPUI cập nhật hit-test theo WM_MOUSEMOVE, OS hỏi
    // WM_NCHITTEST, rồi gửi WM_NC* (nút tiêu đề, vùng kéo) hoặc WM_LBUTTON* (HTCLIENT). Trả mã hit-test.
    public static int Click(IntPtr h, int x, int y, int hover) {
        IntPtr r;
        var p = new POINT { X = x, Y = y };
        ClientToScreen(h, ref p);
        PostMessageW(h, 0x0200, IntPtr.Zero, Pack(x, y)); // WM_MOUSEMOVE
        System.Threading.Thread.Sleep(hover);
        SendMessageTimeoutW(h, 0x0084, IntPtr.Zero, Pack(p.X, p.Y), 2, 2000, out r); // WM_NCHITTEST
        int ht = (int)r;
        if (ht == 1) { // HTCLIENT
            PostMessageW(h, 0x0201, (IntPtr)1, Pack(x, y));
            System.Threading.Thread.Sleep(60);
            PostMessageW(h, 0x0202, IntPtr.Zero, Pack(x, y));
        } else {
            PostMessageW(h, 0x00A0, (IntPtr)ht, Pack(p.X, p.Y)); // WM_NCMOUSEMOVE
            System.Threading.Thread.Sleep(60);
            PostMessageW(h, 0x00A1, (IntPtr)ht, Pack(p.X, p.Y)); // WM_NCLBUTTONDOWN
            System.Threading.Thread.Sleep(60);
            PostMessageW(h, 0x00A2, (IntPtr)ht, Pack(p.X, p.Y)); // WM_NCLBUTTONUP
        }
        return ht;
    }

    // Một bước của thao tác kéo: 0 nhấn, 1 rê, 2 thả. Vùng không phải client đi đường WM_NC* như chuột thật (nhấn trên
    // HTCAPTION thì GPUI vẫn nhận MouseDown trước khi OS dời cửa sổ). Trả mã hit-test.
    public static int Drag(IntPtr h, int x, int y, int kind, int hover) {
        IntPtr r;
        var p = new POINT { X = x, Y = y };
        ClientToScreen(h, ref p);
        if (kind == 0) { PostMessageW(h, 0x0200, IntPtr.Zero, Pack(x, y)); System.Threading.Thread.Sleep(hover); } // rê tới trước khi nhấn
        SendMessageTimeoutW(h, 0x0084, IntPtr.Zero, Pack(p.X, p.Y), 2, 2000, out r);
        int ht = (int)r;
        IntPtr held = kind == 2 ? IntPtr.Zero : (IntPtr)1;
        if (kind == 1) held = (IntPtr)Held;
        uint[] client = { 0x0201, 0x0200, 0x0202 }; // WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_LBUTTONUP
        uint[] nc = { 0x00A1, 0x00A0, 0x00A2 };
        if (ht == 1) PostMessageW(h, client[kind], held, Pack(x, y));
        else PostMessageW(h, nc[kind], (IntPtr)ht, Pack(p.X, p.Y));
        if (kind == 0) Held = 1;
        if (kind == 2) Held = 0;
        System.Threading.Thread.Sleep(hover);
        return ht;
    }
    static int Held;

    // Rê tới (x, y) rồi hỏi WM_NCHITTEST, không nhấp.
    public static int HitTest(IntPtr h, int x, int y, int hover) {
        IntPtr r;
        var p = new POINT { X = x, Y = y };
        ClientToScreen(h, ref p);
        PostMessageW(h, 0x0200, IntPtr.Zero, Pack(x, y));
        System.Threading.Thread.Sleep(hover);
        SendMessageTimeoutW(h, 0x0084, IntPtr.Zero, Pack(p.X, p.Y), 2, 2000, out r);
        return (int)r;
    }

    // Nhấp đúp như chuột thật ở vùng không phải client: WM_NCLBUTTONDBLCLK thay lần nhấn thứ hai.
    public static int DoubleClick(IntPtr h, int x, int y, int hover) {
        int ht = HitTest(h, x, y, hover);
        var p = new POINT { X = x, Y = y };
        ClientToScreen(h, ref p);
        if (ht != 1) {
            PostMessageW(h, 0x00A3, (IntPtr)ht, Pack(p.X, p.Y)); // WM_NCLBUTTONDBLCLK
            System.Threading.Thread.Sleep(60);
            PostMessageW(h, 0x00A2, (IntPtr)ht, Pack(p.X, p.Y));
        }
        return ht;
    }
    [DllImport("kernel32", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool CreateProcessW(string app, StringBuilder cmd, IntPtr pa, IntPtr ta, bool inherit, uint flags,
        IntPtr env, string cwd, ref STARTUPINFO si, out PROCESS_INFORMATION pi);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }

    public static List<IntPtr> Windows(IntPtr desktop, int pid) {
        var found = new List<IntPtr>();
        EnumDesktopWindows(desktop, (h, _) => {
            int owner; GetWindowThreadProcessId(h, out owner);
            if (owner == pid && IsWindowVisible(h)) found.Add(h);
            return true;
        }, IntPtr.Zero);
        return found;
    }
    public static string Title(IntPtr h) { var s = new StringBuilder(256); GetWindowTextW(h, s, 256); return s.ToString(); }
}
'@

$exe = (Resolve-Path $Exe).Path
# Dữ liệu tạm cố định cạnh exe (`<target>/hidden-data`), dùng lại mọi lượt: xoá sạch lúc đầu lượt, không rải thư mục GUID vào
# %TEMP% (SQLite còn mở lúc dọn ⇒ xoá thất bại im lặng, thư mục sót gom lại làm đăng nhập Windows chậm).
$data = Join-Path (Split-Path (Split-Path $exe)) "hidden-data"
for ($i = 0; (Test-Path $data) -and $i -lt 40; $i++) {
    Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
    if (Test-Path $data) { Start-Sleep -Milliseconds 100 }
}
if (Test-Path $data) { throw "$data is still in use (another groove from a previous run?)" }
New-Item -ItemType Directory -Force $data | Out-Null

# Bản phát hành thật bỏ qua GROOVE_DATA_DIR, trình biên dịch bỏ luôn chuỗi này khỏi exe: không có chuỗi là từ chối, trước khi chạy.
$bytes = [Text.Encoding]::Latin1.GetString([IO.File]::ReadAllBytes($exe))
if ($bytes.IndexOf("GROOVE_DATA_DIR", [StringComparison]::Ordinal) -lt 0) {
    throw "$exe does not honour GROOVE_DATA_DIR and would open the real library. Build with debug assertions."
}
Remove-Variable bytes
if ($Settings) { Set-Content -LiteralPath (Join-Path $data "settings.json") -Value $Settings -Encoding utf8NoBOM }
if ($Seed) { & (Resolve-Path $Seed).Path $data $Songs | Out-Host; if ($LASTEXITCODE) { throw "seed failed: $LASTEXITCODE" } }

$name = "groove-hidden-" + $PID
$desktop = [HiddenDesktop]::CreateDesktopW($name, [IntPtr]::Zero, [IntPtr]::Zero, 0, 0x10000000, [IntPtr]::Zero)
if ($desktop -eq [IntPtr]::Zero) { throw "CreateDesktop failed: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())" }

$foreground = [HiddenDesktop]::GetForegroundWindow()
$env:GROOVE_DATA_DIR = $data
$env:GROOVE_LOG = "info"
$env:RUST_BACKTRACE = "1"
$si = New-Object HiddenDesktop+STARTUPINFO
$si.cb = [Runtime.InteropServices.Marshal]::SizeOf($si)
$si.lpDesktop = "WinSta0\$name"
$pi = New-Object HiddenDesktop+PROCESS_INFORMATION
$line = if ($Active) { "`"$exe`"" } else { "`"$exe`" --background" }
if ($Keys) { $line += " --keys `"$Keys`"" }
if ($Pip) { $line += " --pip" }
$cmd = New-Object Text.StringBuilder $line
# CREATE_NO_WINDOW: bản debug là ứng dụng console, không để console hiện ở desktop đang dùng.
if (-not [HiddenDesktop]::CreateProcessW($exe, $cmd, [IntPtr]::Zero, [IntPtr]::Zero, $false, 0x08000000, [IntPtr]::Zero, (Split-Path $exe), [ref]$si, [ref]$pi)) {
    [void][HiddenDesktop]::CloseDesktop($desktop)
    throw "CreateProcess failed: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
}
$proc = Get-Process -Id $pi.dwProcessId
$script:cpuMark = [TimeSpan]::Zero
$script:wallMark = [Diagnostics.Stopwatch]::StartNew()

# Cửa sổ cần bấm / chụp: `-Pip` lấy cửa sổ nhỏ nhất (PiP 420 × 140), không thì cửa sổ đầu.
function Target-Window($list) {
    if (-not $Pip -or $list.Count -lt 2) { return $list[0] }
    $list | Sort-Object { $r = New-Object HiddenDesktop+RECT; [void][HiddenDesktop]::GetWindowRect($_, [ref]$r); ($r.R - $r.L) * ($r.B - $r.T) } |
        Select-Object -First 1
}

function Take-Shot($h, [string]$path) {
    $rect = New-Object HiddenDesktop+RECT
    [void][HiddenDesktop]::GetWindowRect($h, [ref]$rect)
    $bitmap = New-Object Drawing.Bitmap ([Math]::Max(1, $rect.R - $rect.L)), ([Math]::Max(1, $rect.B - $rect.T))
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    $dc = $graphics.GetHdc()
    [void][HiddenDesktop]::PrintWindow($h, $dc, 2) # PW_RENDERFULLCONTENT
    $graphics.ReleaseHdc($dc)
    $graphics.Dispose()
    $bitmap.Save($path)
    $bitmap.Dispose()
    "shot: $path"
}

function Measure-Now([string]$label) {
    $proc.Refresh()
    if ($proc.HasExited) { "${label}: exited with code $($proc.ExitCode)"; return }
    $win = [HiddenDesktop]::Windows($desktop, $proc.Id)
    $title = if ($win.Count) { [HiddenDesktop]::Title($win[0]) } else { "" }
    $cpuNow = $proc.TotalProcessorTime
    $cpu = 100 * ($cpuNow - $script:cpuMark).TotalMilliseconds / [Math]::Max(1, $script:wallMark.ElapsedMilliseconds)
    $script:cpuMark = $cpuNow; $script:wallMark.Restart()
    "{0}: working_set={1:N1} MB private={2:N1} MB peak_ws={3:N1} MB cpu={7:N1}% threads={4} windows={5} title='{6}'" -f $label,
        ($proc.WorkingSet64 / 1MB), ($proc.PrivateMemorySize64 / 1MB), ($proc.PeakWorkingSet64 / 1MB), $proc.Threads.Count, $win.Count, $title, $cpu
}

try {
    $step = [Math]::Max(1, [int]($Seconds / 3))
    for ($t = $step; $t -le $Seconds; $t += $step) {
        Start-Sleep -Seconds $step
        Measure-Now "t=${t}s"
    }
    $win = [HiddenDesktop]::Windows($desktop, $proc.Id)
    if ($Clicks -and $win.Count) {
        $h = Target-Window $win
        # Tâm nút tính từ mép phải, theo px logic (nút rộng 36, thanh cao 36); y = 24 tránh dải kéo giãn mép trên.
        $offset = @{ min = 90; max = 54; close = 18 }
        $names = @{ 1 = "HTCLIENT"; 2 = "HTCAPTION"; 10 = "HTLEFT"; 11 = "HTRIGHT"; 15 = "HTBOTTOM"; 16 = "HTBOTTOMLEFT"; 17 = "HTBOTTOMRIGHT"; 13 = "HTTOPLEFT"; 8 = "HTMINBUTTON"; 9 = "HTMAXBUTTON"; 20 = "HTCLOSE"; 12 = "HTTOP"; 14 = "HTTOPRIGHT" }
        foreach ($c in $Clicks.Split(",")) {
            $scale = [HiddenDesktop]::GetDpiForWindow($h) / 96.0
            $client = New-Object HiddenDesktop+RECT
            [void][HiddenDesktop]::GetClientRect($h, [ref]$client)
            if ($c -match '^shot@(.+)$') { Take-Shot $h $Matches[1]; continue }
            if ($c -eq "rects") {
                $wr = New-Object HiddenDesktop+RECT; [void][HiddenDesktop]::GetWindowRect($h, [ref]$wr)
                "rects: window={0}x{1} client={2}x{3}" -f ($wr.R - $wr.L), ($wr.B - $wr.T), $client.R, $client.B
                continue
            }
            if ($c -eq "settings") { Start-Sleep -Milliseconds 1500; "settings: " + (Get-Content -Raw (Join-Path $data "settings.json") -ErrorAction SilentlyContinue); continue }
            if ($c -match '^nudge@(\d+)$') {
                # Dời cửa sổ n lần, 1 px mỗi 16 ms như lúc kéo (mỗi lần một WM_MOVE), in CPU dùng trong lúc dời.
                $r = New-Object HiddenDesktop+RECT; [void][HiddenDesktop]::GetWindowRect($h, [ref]$r)
                $proc.Refresh(); $cpu0 = $proc.TotalProcessorTime; $wall = [Diagnostics.Stopwatch]::StartNew()
                $ignored = [IntPtr]::Zero
                [void][HiddenDesktop]::SendMessageTimeoutW($h, 0x0231, [IntPtr]::Zero, [IntPtr]::Zero, 2, 2000, [ref]$ignored) # WM_ENTERSIZEMOVE
                for ($i = 1; $i -le [int]$Matches[1]; $i++) {
                    [void][HiddenDesktop]::SetWindowPos($h, [IntPtr]::Zero, $r.L + ($i % 40), $r.T, 0, 0, 0x0015) # NOSIZE | NOZORDER | NOACTIVATE
                    Start-Sleep -Milliseconds 16
                }
                [void][HiddenDesktop]::SendMessageTimeoutW($h, 0x0232, [IntPtr]::Zero, [IntPtr]::Zero, 2, 2000, [ref]$ignored) # WM_EXITSIZEMOVE
                $proc.Refresh()
                "{0}: cpu={1:N1}% over {2:N0} ms" -f $c, (100 * ($proc.TotalProcessorTime - $cpu0).TotalMilliseconds / $wall.ElapsedMilliseconds), $wall.ElapsedMilliseconds
                continue
            }
            if ($c -match '^(down|move|up)@(-?\d+):(-?\d+)$') {
                $lx = [int]$Matches[2]
                $x = if ($lx -lt 0) { [int]($client.R + $lx * $scale) } else { [int]($lx * $scale) }
                $ly = [int]$Matches[3]
                $y = if ($ly -lt 0) { [int]($client.B + $ly * $scale) } else { [int]($ly * $scale) }
                $ht = [HiddenDesktop]::Drag($h, $x, $y, @{ down = 0; move = 1; up = 2 }[$Matches[1]], $Hover)
                "{0}: hit_test={1}" -f $c, $(if ($names.ContainsKey($ht)) { $names[$ht] } else { $ht })
                continue
            }
            if ($c -match '^(hit|dbl)@(-?\d+):(-?\d+)$') {
                $lx = [int]$Matches[2]
                $x = if ($lx -lt 0) { [int]($client.R + $lx * $scale) } else { [int]($lx * $scale) }
                $ly = [int]$Matches[3]
                $y = if ($ly -lt 0) { [int]($client.B + $ly * $scale) } else { [int]($ly * $scale) }
                $ht = if ($Matches[1] -eq "hit") { [HiddenDesktop]::HitTest($h, $x, $y, $Hover) } else { [HiddenDesktop]::DoubleClick($h, $x, $y, $Hover) }
            } else {
                $x = [int]($client.R - $offset[$c] * $scale); $y = [int](24 * $scale)
                $ht = [HiddenDesktop]::Click($h, $x, $y, $Hover)
            }
            Start-Sleep -Milliseconds 700
            $alive = [HiddenDesktop]::IsWindow($h)
            "click {0} at {1},{2}: hit_test={3} minimized={4} maximized={5} window_alive={6}" -f $c, $x, $y,
                $(if ($names.ContainsKey($ht)) { $names[$ht] } else { $ht }),
                ($alive -and [HiddenDesktop]::IsIconic($h)), ($alive -and [HiddenDesktop]::IsZoomed($h)), $alive
            if ($alive -and [HiddenDesktop]::IsIconic($h)) { [void][HiddenDesktop]::ShowWindowAsync($h, 9); Start-Sleep -Milliseconds 500 } # SW_RESTORE
        }
        $win = [HiddenDesktop]::Windows($desktop, $proc.Id)
    }
    if ($Shot -and $win.Count) { Take-Shot (Target-Window $win) $Shot }
} finally {
    $proc.Refresh()
    if (-not $proc.HasExited) { $proc.Kill(); $proc.WaitForExit(5000) | Out-Null }
    [void][HiddenDesktop]::CloseDesktop($desktop)
    "foreground_unchanged=$([HiddenDesktop]::GetForegroundWindow() -eq $foreground)"
    $logs = Join-Path $data "logs"
    if (Test-Path $logs) {
        "data_dir_used=True"
        Get-ChildItem $logs -File | ForEach-Object { Select-String -Path $_.FullName -Pattern "WARN|ERROR|panic" | ForEach-Object { $_.Line } } | Select-Object -First 30
    } else {
        "data_dir_used=False (exe không có debug assertions: có thể đã mở dữ liệu thật, đừng dùng exe này)"
    }
}
