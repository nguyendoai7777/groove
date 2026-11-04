<#
.SYNOPSIS
  Dựng bản cài Windows `<OutDir>/groovex-<version>-setup.exe` (NSIS): build release → `cargo deny` + trang
  giấy phép → `makensis`. Bước nào đỏ thì dừng. Không ký mã: SmartScreen sẽ cảnh báo (ký là dịch vụ trả phí, chủ dự án quyết).

.EXAMPLE
  winget install NSIS.NSIS; cargo install cargo-deny --locked; cargo install cargo-about --locked --features cli
  pwsh packaging/windows/build-installer.ps1
  pwsh packaging/windows/build-installer.ps1 -OutDir ../release/groove -Makensis ../../tools/nsis/makensis.exe
#>
param(
    [string] $OutDir,
    [string] $Makensis,
    [switch] $SkipBuild
)
$ErrorActionPreference = 'Stop'

$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if (-not $OutDir) { $OutDir = Join-Path $repo 'dist' }
New-Item -ItemType Directory -Force $OutDir | Out-Null
$OutDir = (Resolve-Path $OutDir).Path
$stage = Join-Path $OutDir 'groove-stage'

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
$nsis = Find-Makensis

$metadata = cargo metadata --format-version 1 --no-deps --manifest-path (Join-Path $repo 'Cargo.toml') | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
$version = ($metadata.packages | Where-Object name -EQ 'groove-app').version
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw "version $version does not fit VIProductVersion" }
$exe = Join-Path $metadata.target_directory 'release/groove.exe'

if (-not $SkipBuild) {
    Push-Location $repo
    try {
        cargo build --release --locked -p groove-app --bin groove
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

    $setup = Join-Path $OutDir "groovex-$version-setup.exe"
    & $nsis /V2 /WX /INPUTCHARSET UTF8 `
        "/DVERSION=$version" "/DVERSION4=$version.0" "/DSTAGE=$stage" `
        "/DICON=$(Join-Path $repo 'crates/groove-app/assets/app/groove.ico')" "/DOUTFILE=$setup" `
        (Join-Path $repo 'packaging/windows/groove.nsi')
    if ($LASTEXITCODE -ne 0) { throw "makensis failed (exit $LASTEXITCODE)" }
} finally {
    Remove-Item -Recurse -Force -LiteralPath $stage -ErrorAction SilentlyContinue
}

$sha = (Get-FileHash -Algorithm SHA256 -LiteralPath $setup).Hash.ToLowerInvariant()
Set-Content -Encoding ascii -LiteralPath "$setup.sha256" -Value "$sha  $(Split-Path -Leaf $setup)"
Write-Host ("{0}: {1:N1} MB, sha256 {2}" -f $setup, ((Get-Item -LiteralPath $setup).Length / 1MB), $sha)
