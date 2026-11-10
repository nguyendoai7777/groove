# Build bản cài Groovex cho máy đang chạy, ra <gốc>/release/groove/. Gốc workspace gọi sang đây (`pwsh ./build.ps1 -Project groove`);
# chạy thẳng cũng được:
#   pwsh groove/build.ps1 [-f]     # -Force: biên dịch lại mã của ta, giữ thư viện đã build
#   pwsh groove/build.ps1 -Package setup,portable   # không hỏi dạng đóng gói
# Windows: hỏi dạng đóng gói (ô tích): bản cài NSIS `_setup.exe` (tích sẵn) và / hoặc bản chạy thẳng `_portable.zip`
# (packaging/windows/build-installer.ps1). Windows chỉ build x64 (chủ dự án 10/11/2026: phần lớn PC Windows là Intel / AMD, bỏ ARM).
# macOS / Linux: chuyển sang build.sh (.app + .dmg / .deb).
param(
    [Alias('f')] [switch] $Force,
    # `setup`, `portable` (cách bằng dấu phẩy được: `pwsh -File` từ build.sh truyền một chuỗi).
    [string[]] $Package
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
if ($Package | Where-Object { $_ -notin 'setup', 'portable' }) { throw "-Package chỉ nhận setup, portable (nhận: $($Package -join ', '))" }

if (-not $IsWindows) {
    bash (Join-Path $repo 'build.sh') @(if ($Force) { '--force' })
    exit $LASTEXITCODE
}
if ($Force) {
    cargo clean --release --manifest-path (Join-Path $repo 'Cargo.toml') -p groove-app -p groove -p groove-audio -p groove-core -p groove-library -p materiant
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
if (-not $Package) {
    $Package = @(Select-Many 'Đóng gói:' @(
            (Option 'setup' 'Bản cài (_setup.exe)'), (Option 'portable' 'Bản chạy thẳng, không cài (_portable.zip)')) @('setup'))
}
& (Join-Path $repo 'packaging/windows/build-installer.ps1') -OutDir (Join-Path $root 'release/groove') -Package $Package
exit 0
