<#
.SYNOPSIS
  Dựng bản Windows x64 ở <OutDir>: `groovex_<version>_win_x64_setup.exe` (bản cài NSIS) và / hoặc
  `groovex_<version>_win_x64_portable.zip` (bản chạy thẳng: giải nén ra một thư mục, chạy `groove.exe`), chọn bằng
  `-Package setup,portable` (mặc định chỉ bản cài); mỗi tệp kèm `.sha256`. Build release → `cargo deny` + trang giấy phép →
  `makensis` / nén zip. Bước nào đỏ thì dừng. Không ký mã: SmartScreen sẽ cảnh báo (ký là dịch vụ trả phí, chủ dự án quyết).

.EXAMPLE
  winget install NSIS.NSIS; cargo install cargo-deny --locked; cargo install cargo-about --locked --features cli
  pwsh packaging/windows/build-installer.ps1
  pwsh packaging/windows/build-installer.ps1 -OutDir ../release/groove -Makensis ../../tools/nsis/makensis.exe
#>
param(
    [string] $OutDir,
    [string] $Makensis,
    [switch] $SkipBuild,
    [ValidateSet('setup', 'portable')] [string[]] $Package = @('setup')
)
$ErrorActionPreference = 'Stop'

$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if (-not $OutDir) { $OutDir = Join-Path $repo 'dist' }
New-Item -ItemType Directory -Force $OutDir | Out-Null
$OutDir = (Resolve-Path $OutDir).Path

function Find-Makensis {
    if ($Makensis) { return (Resolve-Path $Makensis).Path }
    $onPath = Get-Command makensis -ErrorAction Ignore
    if ($onPath) { return $onPath.Source }
    # Worktree nằm ở `.claude/worktrees/…`: tìm theo checkout chính của repo (`<…>/groove/.git`), không theo thư mục đang đứng.
    $main = git -C $repo rev-parse --path-format=absolute --git-common-dir 2>$null
    $known = @(
        (Join-Path $repo '../../tools/nsis/makensis.exe'),
        $(if ($main) { Join-Path $main '../../../tools/nsis/makensis.exe' }),
        (Join-Path ${env:ProgramFiles(x86)} 'NSIS/makensis.exe'),
        (Join-Path $env:ProgramFiles 'NSIS/makensis.exe')
    )
    foreach ($path in ($known | Where-Object { $_ })) { if (Test-Path -LiteralPath $path) { return (Resolve-Path $path).Path } }
    throw 'makensis not found: winget install NSIS.NSIS, or pass -Makensis <path>'
}
$nsis = if ('setup' -in $Package) { Find-Makensis }

$metadata = cargo metadata --format-version 1 --no-deps --manifest-path (Join-Path $repo 'Cargo.toml') | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
$version = ($metadata.packages | Where-Object name -EQ 'groove-app').version
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw "version $version does not fit VIProductVersion" }
$hostArch = if ([Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq 'Arm64') { 'arm64' } else { 'x64' }
# Chỉ phát hành x64 (bỏ ARM, chủ dự án 10/11/2026). Máy ARM ⇒ build chéo bằng `--target`, exe ở `<target>/<triple>/release`.
$arch = 'x64'
$triple = if ($hostArch -ne 'x64') { 'x86_64-pc-windows-msvc' }
$exe = Join-Path $metadata.target_directory "$(if ($triple) { "$triple/" })release/groove.exe"
# Tên tệp: {app}_{version}_{os}_{arch}; thư mục dựng mang đúng tên này để zip giải nén ra một thư mục cùng tên.
$baseName = "groovex_${version}_win_$arch"
$stage = Join-Path $OutDir $baseName
$outputs = @()
$setup = $null

if (-not $SkipBuild) {
    Push-Location $repo
    try {
        if ($triple -and (rustup target list --installed) -notcontains $triple) {
            rustup target add $triple
            if ($LASTEXITCODE -ne 0) { throw "rustup target add $triple failed" }
        }
        cargo build --release --locked -p groove-app --bin groove @(if ($triple) { '--target', $triple })
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }
    } finally {
        Pop-Location
    }
}


if (Test-Path -LiteralPath $stage) { Remove-Item -Recurse -Force -LiteralPath $stage }
New-Item -ItemType Directory -Force $stage | Out-Null
try {
    Copy-Item -LiteralPath $exe -Destination $stage
    & (Join-Path $repo 'packaging/licenses/gen-licenses.ps1') -Out (Join-Path $stage 'licenses')

    if ('setup' -in $Package) {
        $setup = Join-Path $OutDir "${baseName}_setup.exe"
        & $nsis /V2 /WX /INPUTCHARSET UTF8 `
            "/DVERSION=$version" "/DVERSION4=$version.0" "/DSTAGE=$stage" `
            "/DICON=$(Join-Path $repo 'crates/groove-app/assets/app/groove.ico')" "/DOUTFILE=$setup" `
            (Join-Path $repo 'packaging/windows/groove.nsi')
        if ($LASTEXITCODE -ne 0) { throw "makensis failed (exit $LASTEXITCODE)" }
        $outputs += $setup
    }
    if ('portable' -in $Package) {
        $zip = Join-Path $OutDir "${baseName}_portable.zip"
        Remove-Item -Force -LiteralPath $zip -ErrorAction Ignore
        Compress-Archive -LiteralPath $stage -DestinationPath $zip -CompressionLevel Optimal
        $outputs += $zip
    }
} finally {
    Remove-Item -Recurse -Force -LiteralPath $stage -ErrorAction SilentlyContinue
}

foreach ($file in $outputs) {
    $sha = (Get-FileHash -Algorithm SHA256 -LiteralPath $file).Hash.ToLowerInvariant()
    # Xuống dòng LF, không CRLF: `sha256sum -c` (Linux / macOS / Git Bash) báo không đọc được tệp nếu dòng có CR.
    [IO.File]::WriteAllText("$file.sha256", "$sha  $(Split-Path -Leaf $file)`n")
    Write-Host ("{0}: {1:N1} MB, sha256 {2}" -f $file, ((Get-Item -LiteralPath $file).Length / 1MB), $sha)
}
# Lệnh đưa bản cài lên kênh cập nhật (groove-gd2_05, server `cloud`): app tự tải `_setup.exe` rồi chạy `/S /UPDATE`.
if ($setup -and (Test-Path -LiteralPath $setup)) {
    Write-Host ""
    Write-Host "Đưa lên kênh cập nhật của staging (đứng ở gốc xdesktop, Git Bash):"
    Write-Host ("  bash cloud/deploy/publish-release.sh groove {0} --version {1} --os windows --arch x64 --plan free --notes groove/release-notes/v{1}.md" -f ($setup -replace '\\', '/'), $version)
}
