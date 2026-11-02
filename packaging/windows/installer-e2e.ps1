<#
.SYNOPSIS
  Soát bản cài thật, không mở app: cài im lặng vào thư mục tạm → tệp, icon exe, khoá registry → exe bị giữ thì cài đè dừng mã 2 →
  gỡ im lặng → registry về như cũ. Ghi HKCU của máy đang chạy rồi dọn; máy đã cài Groove X thì từ chối, không đụng bản đang dùng.

.EXAMPLE
  pwsh packaging/windows/installer-e2e.ps1 -Setup dist/groove-x-0.2.0-setup.exe
#>
param([Parameter(Mandatory)] [string] $Setup)
$ErrorActionPreference = 'Stop'
$Setup = (Resolve-Path $Setup).Path

$classes = 'HKCU:\Software\Classes'
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\GrooveX'
$progId = 'GrooveX.AudioFile'
$extensions = 'mp3', 'flac', 'wav', 'm4a', 'm4r', 'ogg'
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Groove X.lnk'

if ((Test-Path $uninstallKey) -or (Test-Path "$classes\$progId") -or (Test-Path $shortcut)) { throw 'Groove X is installed on this machine: not touching it' }

function Check([string] $Name, [bool] $Ok) {
    Write-Host ("{0,-4} {1}" -f $(if ($Ok) { 'ok' } else { 'FAIL' }), $Name)
    if (-not $Ok) { $script:failed = $true }
}
function Snapshot {
    foreach ($ext in $extensions) {
        $key = "$classes\.$ext"
        $values = if (Test-Path "$key\OpenWithProgids") { (Get-Item "$key\OpenWithProgids").Property -join ',' } else { '-' }
        $default = if (Test-Path $key) { (Get-Item $key).GetValue('') } else { '-' }
        "$ext default=$default open=$values"
    }
}
function Run([string] $Exe, [string[]] $Arguments) {
    (Start-Process -FilePath $Exe -ArgumentList $Arguments -Wait -PassThru).ExitCode
}

$before = Snapshot
$dir = Join-Path ([IO.Path]::GetTempPath()) "groove-e2e-$([guid]::NewGuid().ToString('N'))"
$installed = Join-Path $dir 'Groove X'
$failed = $false
try {
    # `/D=` phải đứng cuối, không ngoặc kép, được phép có dấu cách.
    Check 'install exit 0' ((Run $Setup @('/S', "/D=$installed")) -eq 0)
    $exe = Join-Path $installed 'groove.exe'
    Check 'groove.exe' (Test-Path $exe)
    Check 'uninstall.exe' (Test-Path (Join-Path $installed 'uninstall.exe'))
    Check 'third-party.html' (Test-Path (Join-Path $installed 'licenses/third-party.html'))
    $page = Get-Content -Raw -Encoding utf8 (Join-Path $installed 'licenses/third-party.html')
    Check 'page names libasound' ($page.Contains('data-component="ALSA library (libasound)"'))
    Check 'no libasound file' (-not (Get-ChildItem $installed -Recurse -Filter 'libasound*'))
    Check 'shortcut' (Test-Path $shortcut)

    Add-Type -Namespace GrooveE2e -Name Shell -MemberDefinition '[DllImport("shell32.dll", CharSet = CharSet.Unicode)] public static extern uint ExtractIconExW(string file, int index, IntPtr large, IntPtr small, uint count);'
    Check 'exe has an icon' ([GrooveE2e.Shell]::ExtractIconExW($exe, -1, [IntPtr]::Zero, [IntPtr]::Zero, 0) -ge 1)

    Check 'open command' ((Get-Item "$classes\$progId\shell\open\command").GetValue('') -eq "`"$exe`" `"%1`"")
    foreach ($ext in $extensions) {
        Check ".$ext in OpenWithProgids" ((Get-Item "$classes\.$ext\OpenWithProgids").Property -contains $progId)
    }
    $default = @($before | ForEach-Object { ($_ -split ' open=')[0] })
    $now = @(Snapshot | ForEach-Object { ($_ -split ' open=')[0] })
    Check 'default association untouched' (($default -join ';') -eq ($now -join ';'))

    $hold = [IO.File]::Open($exe, 'Open', 'Read', 'Read')
    try {
        Check 'upgrade over a locked exe stops with 2' ((Run $Setup @('/S', "/D=$installed")) -eq 2)
        Check 'uninstall with a locked exe stops with 2' ((Run (Join-Path $installed 'uninstall.exe') @('/S', "_?=$installed")) -eq 2)
        Check 'still registered' (Test-Path $uninstallKey)
    } finally {
        $hold.Dispose()
    }
    Check 'upgrade after closing exit 0' ((Run $Setup @('/S', "/D=$installed")) -eq 0)

    # `_?=`: chạy tại chỗ, không chép sang thư mục tạm, nên chờ được nó xong.
    Check 'uninstall exit 0' ((Run (Join-Path $installed 'uninstall.exe') @('/S', "_?=$installed")) -eq 0)
    Check 'files gone' (-not (Test-Path $exe) -and -not (Test-Path (Join-Path $installed 'licenses')))
    Check 'shortcut gone' (-not (Test-Path $shortcut))
    Check 'uninstall key gone' (-not (Test-Path $uninstallKey))
    Check 'ProgID gone' (-not (Test-Path "$classes\$progId"))
    Check 'Applications key gone' (-not (Test-Path "$classes\Applications\groove.exe"))
    Check 'associations as before' (($before -join ';') -eq ((Snapshot) -join ';'))
} finally {
    Remove-Item -Recurse -Force -LiteralPath $dir -ErrorAction SilentlyContinue
}
if ($failed) { exit 1 }
Write-Host 'all ok'
