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
# In: số cửa sổ, RAM (working set, private) ở mỗi mốc, dòng log WARN / ERROR, `foreground_unchanged`.
param(
    [Parameter(Mandatory)][string]$Exe,
    [int]$Seconds = 15,
    [string]$Keys = "",
    [string]$Shot = ""
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
$data = Join-Path ([IO.Path]::GetTempPath()) ("groove-hidden-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force $data | Out-Null

# Bản phát hành thật bỏ qua GROOVE_DATA_DIR, trình biên dịch bỏ luôn chuỗi này khỏi exe: không có chuỗi là từ chối, trước khi chạy.
$bytes = [Text.Encoding]::Latin1.GetString([IO.File]::ReadAllBytes($exe))
if ($bytes.IndexOf("GROOVE_DATA_DIR", [StringComparison]::Ordinal) -lt 0) {
    Remove-Item -Recurse -Force $data
    throw "$exe does not honour GROOVE_DATA_DIR and would open the real library. Build with debug assertions."
}
Remove-Variable bytes

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
$line = "`"$exe`" --background"
if ($Keys) { $line += " --keys `"$Keys`"" }
$cmd = New-Object Text.StringBuilder $line
# CREATE_NO_WINDOW: bản debug là ứng dụng console, không để console hiện ở desktop đang dùng.
if (-not [HiddenDesktop]::CreateProcessW($exe, $cmd, [IntPtr]::Zero, [IntPtr]::Zero, $false, 0x08000000, [IntPtr]::Zero, (Split-Path $exe), [ref]$si, [ref]$pi)) {
    [void][HiddenDesktop]::CloseDesktop($desktop)
    throw "CreateProcess failed: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
}
$proc = Get-Process -Id $pi.dwProcessId

function Measure-Now([string]$label) {
    $proc.Refresh()
    if ($proc.HasExited) { "${label}: exited with code $($proc.ExitCode)"; return }
    $win = [HiddenDesktop]::Windows($desktop, $proc.Id)
    $title = if ($win.Count) { [HiddenDesktop]::Title($win[0]) } else { "" }
    "{0}: working_set={1:N1} MB private={2:N1} MB peak_ws={3:N1} MB threads={4} windows={5} title='{6}'" -f $label,
        ($proc.WorkingSet64 / 1MB), ($proc.PrivateMemorySize64 / 1MB), ($proc.PeakWorkingSet64 / 1MB), $proc.Threads.Count, $win.Count, $title
}

try {
    $step = [Math]::Max(1, [int]($Seconds / 3))
    for ($t = $step; $t -le $Seconds; $t += $step) {
        Start-Sleep -Seconds $step
        Measure-Now "t=${t}s"
    }
    $win = [HiddenDesktop]::Windows($desktop, $proc.Id)
    if ($Shot -and $win.Count) {
        $rect = New-Object HiddenDesktop+RECT
        [void][HiddenDesktop]::GetWindowRect($win[0], [ref]$rect)
        $bitmap = New-Object Drawing.Bitmap ([Math]::Max(1, $rect.R - $rect.L)), ([Math]::Max(1, $rect.B - $rect.T))
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        $dc = $graphics.GetHdc()
        [void][HiddenDesktop]::PrintWindow($win[0], $dc, 2) # PW_RENDERFULLCONTENT
        $graphics.ReleaseHdc($dc)
        $graphics.Dispose()
        $bitmap.Save($Shot)
        $bitmap.Dispose()
        "shot: $Shot"
    }
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
    Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
}
