<#
.SYNOPSIS
  Chạy thử Groove X mà không làm phiền người đang dùng máy: mở ở màn phụ (máy nhà: DELL), không giành focus, phím đưa
  thẳng vào cửa sổ GPUI, chụp đúng cửa sổ bằng PrintWindow, đọc RAM, rồi đóng. Dữ liệu ở thư mục tạm (GROOVE_DATA_DIR,
  chỉ bản debug), không đụng thư viện thật.

.EXAMPLE
  cargo build -p groove-app
  pwsh tools/run.ps1 -Keys "ctrl-k wait@300 shot@palette"

  In RAM, đường dẫn ảnh, `foreground_unchanged=True|False` (False là app đang dùng bị mất focus: lỗi, phải báo).
  Xem ảnh xong xoá thư mục -Out. Exe: `$env:CARGO_TARGET_DIR\debug\groove.exe`, hoặc truyền -Exe.
#>
param(
  [string]$Keys = '',
  [string]$Display = 'secondary',
  [string]$Out = (Join-Path $env:TEMP 'groove-run'),
  [string]$Exe = '',
  [int]$SettleMs = 1500
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
if ($Display -eq 'secondary' -and [System.Windows.Forms.Screen]::AllScreens.Count -lt 2) {
  throw 'máy chỉ có một màn: không mở cửa sổ thử lên màn đang dùng. Cần thử thật thì truyền -Display primary.'
}
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class GrooveWin {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
'@

if (-not $Exe) {
  $target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $PSScriptRoot '..\target' }
  $Exe = Join-Path $target 'debug\groove.exe'
}
if (-not (Test-Path $Exe)) { throw "không thấy $Exe, chạy: cargo build -p groove-app" }
New-Item -ItemType Directory -Force $Out | Out-Null
$stdout = Join-Path $Out 'stdout.txt'
$foregroundBefore = [GrooveWin]::GetForegroundWindow()
$env:GROOVE_DATA_DIR = Join-Path $Out 'data'

$argList = @('--display', $Display, '--background')
if ($Keys) { $argList += @('--keys', "`"$Keys`"") }
$p = Start-Process $Exe -ArgumentList $argList -PassThru -NoNewWindow -RedirectStandardOutput $stdout `
  -RedirectStandardError (Join-Path $Out 'stderr.txt')
for ($i = 0; $i -lt 200 -and $p.MainWindowHandle -eq 0; $i++) { Start-Sleep -Milliseconds 50; $p.Refresh() }
if ($p.MainWindowHandle -eq 0) { $p.Kill(); throw 'groove không mở được cửa sổ' }
$h = $p.MainWindowHandle

# PW_RENDERFULLCONTENT (2): lấy cả nội dung DirectComposition của GPUI, kể cả khi cửa sổ bị che.
function Save-Window([string]$path) {
  $r = New-Object GrooveWin+RECT; [GrooveWin]::GetWindowRect($h, [ref]$r) | Out-Null
  $bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $hdc = $g.GetHdc()
  $ok = [GrooveWin]::PrintWindow($h, $hdc, 2)
  $g.ReleaseHdc($hdc); $g.Dispose()
  $bmp.Save($path); $bmp.Dispose()
  [pscustomobject]@{ Rect = $r; Printed = $ok }
}

$shots = @()
if ($Keys) {
  $deadline = (Get-Date).AddSeconds(60)
  while ((Get-Date) -lt $deadline) {
    $lines = @(Get-Content -Path $stdout -ErrorAction SilentlyContinue)
    foreach ($m in $lines | Select-String -Pattern '^SHOT (\S+)$') {
      $name = $m.Matches[0].Groups[1].Value
      if ($shots -notcontains $name) { $shots += $name; Save-Window (Join-Path $Out "$name.png") | Out-Null }
    }
    if ($lines -match '^KEYS DONE' -or $p.HasExited) { break }
    Start-Sleep -Milliseconds 30
  }
}
Start-Sleep -Milliseconds $SettleMs

$shot = Join-Path $Out 'final.png'; $r = New-Object GrooveWin+RECT; $printed = $false; $ram = '-'
if (-not $p.HasExited) {
  $final = Save-Window $shot
  $r = $final.Rect; $printed = $final.Printed
  $ram = [math]::Round((Get-CimInstance Win32_PerfRawData_PerfProc_Process -Filter "IDProcess = $($p.Id)").WorkingSetPrivate / 1MB, 1)
}
$foregroundAfter = [GrooveWin]::GetForegroundWindow()
if (-not $p.HasExited) { $p.CloseMainWindow() | Out-Null }
if (-not $p.WaitForExit(5000)) { $p.Kill() }

"window_rect=$($r.L),$($r.T) $($r.R - $r.L)x$($r.B - $r.T)"
"memory_mb=$ram"
"screenshot=$shot printed=$printed"
if ($shots) { "shots=" + (($shots | ForEach-Object { Join-Path $Out "$_.png" }) -join ' ') }
"foreground_unchanged=$($foregroundBefore -eq $foregroundAfter) groove_was_foreground=$($foregroundAfter -eq $h)"
"exit=$($p.ExitCode)"
'--- stdout'; Get-Content $stdout
$err = Get-Content (Join-Path $Out 'stderr.txt') -ErrorAction SilentlyContinue
if ($err) { '--- stderr'; $err | Select-Object -Last 10 }
