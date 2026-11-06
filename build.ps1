# Build bản cài Groovex cho máy đang chạy, ra <gốc>/release/groove/. Gốc workspace gọi sang đây (`pwsh ./build.ps1 -Project groove`);
# chạy thẳng cũng được:
#   pwsh groove/build.ps1 [-f]     # -Force: biên dịch lại mã của ta, giữ thư viện đã build
# Windows: bản cài NSIS (packaging/windows/build-installer.ps1). macOS / Linux: chuyển sang build.sh (.app + .dmg / .deb).
param([Alias('f')] [switch] $Force)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$repo = $PSScriptRoot
$root = $repo
while (-not (Test-Path (Join-Path $root 'tools/script-lib.ps1'))) {
    $root = Split-Path $root
    if (-not $root) { throw "Không thấy gốc workspace xdesktop (tools/script-lib.ps1) phía trên $repo" }
}

if (-not $IsWindows) {
    bash (Join-Path $repo 'build.sh') @(if ($Force) { '--force' })
    exit $LASTEXITCODE
}
if ($Force) {
    cargo clean --release --manifest-path (Join-Path $repo 'Cargo.toml') -p groove-app -p groove -p groove-audio -p groove-core -p groove-library -p materiant
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
& (Join-Path $repo 'packaging/windows/build-installer.ps1') -OutDir (Join-Path $root 'release/groove')
exit $LASTEXITCODE
