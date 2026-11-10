# Build bản cài Groovex cho máy đang chạy, ra <gốc>/release/groove/. Gốc workspace gọi sang đây (`pwsh ./build.ps1 -Project groove`);
# chạy thẳng cũng được:
#   pwsh groove/build.ps1 [-f]     # -Force: biên dịch lại mã của ta, giữ thư viện đã build
#   pwsh groove/build.ps1 -Package setup,portable   # không hỏi dạng đóng gói
# Windows: hỏi dạng đóng gói (ô tích): bản cài NSIS `_setup.exe` (tích sẵn) và / hoặc bản chạy thẳng `_portable.zip`
# (packaging/windows/build-installer.ps1). macOS / Linux: chuyển sang build.sh (.app + .dmg / .deb).
param(
    [Alias('f')] [switch] $Force,
    # `setup`, `portable` (cách bằng dấu phẩy được: `pwsh -File` từ build.sh truyền một chuỗi).
    [string[]] $Package,
    # Kiến trúc: `x64`, `arm64` (cách bằng dấu phẩy được); bỏ trống thì hỏi (ô tích, tích sẵn máy đang chạy).
    [string[]] $Arch
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$repo = $PSScriptRoot
$root = $repo
while (-not (Test-Path (Join-Path $root 'tools/script-lib.ps1'))) {
    $root = Split-Path $root
    if (-not $root) { throw "Không thấy gốc workspace xdesktop (tools/script-lib.ps1) phía trên $repo" }
}

. (Join-Path $root 'tools/script-lib.ps1')

$Package = @($Package | ForEach-Object { $_ -split ',' } | ForEach-Object Trim | Where-Object { $_ })
$Arch = @($Arch | ForEach-Object { $_ -split ',' } | ForEach-Object Trim | Where-Object { $_ })
if ($Arch | Where-Object { $_ -notin 'x64', 'arm64' }) { throw "-Arch chỉ nhận x64, arm64 (nhận: $($Arch -join ', '))" }
if ($Package | Where-Object { $_ -notin 'setup', 'portable' }) { throw "-Package chỉ nhận setup, portable (nhận: $($Package -join ', '))" }

if (-not $IsWindows) {
    bash (Join-Path $repo 'build.sh') @(if ($Force) { '--force' })
    exit $LASTEXITCODE
}
if ($Force) {
    cargo clean --release --manifest-path (Join-Path $repo 'Cargo.toml') -p groove-app -p groove -p groove-audio -p groove-core -p groove-library -p materiant
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
if (-not $Arch) {
    $hostArch = if ([Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq 'Arm64') { 'arm64' } else { 'x64' }
    $Arch = @(Select-Many 'Kiến trúc:' @((Option 'x64' 'x64 (64-bit Intel / AMD)'), (Option 'arm64' 'arm64 (64-bit ARM)')) @($hostArch))
}
if (-not $Package) {
    $Package = @(Select-Many 'Đóng gói:' @(
            (Option 'setup' 'Bản cài (_setup.exe)'), (Option 'portable' 'Bản chạy thẳng, không cài (_portable.zip)')) @('setup'))
}
$failed = @()
foreach ($a in $Arch) {
    if ($Arch.Count -gt 1) { Write-Host "`n======== windows $a ========" }
    try {
        & (Join-Path $repo 'packaging/windows/build-installer.ps1') -OutDir (Join-Path $root 'release/groove') -Package $Package -Arch $a
    } catch {
        Write-Error "${a}: $_" -ErrorAction Continue
        $failed += $a
    }
}
if ($failed) { Write-Error "Build lỗi: $($failed -join ', ')." -ErrorAction Continue; exit 1 }
exit 0
