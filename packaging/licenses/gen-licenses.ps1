<#
.SYNOPSIS
  Sinh `<Out>/third-party.html` (D-2): cây phụ thuộc của 6 target phát hành (`about.toml`, cùng danh sách `deny.toml`), gộp theo
  giấy phép, đủ văn bản, cộng thứ cargo không thấy (mảng `$extra` dưới đây). `cargo deny check licenses` đỏ ⇒ dừng.

.EXAMPLE
  cargo install cargo-about --locked --features cli
  pwsh packaging/licenses/gen-licenses.ps1 -Out dist/licenses

  Chạy lại ra đúng cùng nội dung (không có giờ, không có đường dẫn máy).
#>
param(
    [Parameter(Mandatory)] [string] $Out
)
$ErrorActionPreference = 'Stop'

$dir = $PSScriptRoot
$repo = (Resolve-Path (Join-Path $dir '../..')).Path

# G-2: `libasound` là ngoại lệ LGPL duy nhất, chỉ link động và không nằm trong bản cài, nên có mặt trên trang mà không kèm tệp.
$extra = @(
    @{
        Name = 'ALSA library (libasound)'; Version = 'provided by the system'; License = 'LGPL-2.1-or-later'
        Url = 'https://github.com/alsa-project/alsa-lib'; Text = 'libasound.txt'
    },
    @{
        Name = 'Lucide'; Version = 'icons used by the materiant library'; License = 'ISC'
        Url = 'https://lucide.dev'; Text = '../../../materiant/assets/icons/LICENSE-lucide'
    }
)
# Nhãn được phép ngoài danh sách D-2: chỉ đúng thành phần G-2 đã chấp nhận.
$lgplAllowed = 'ALSA library (libasound)'

function Get-Targets([string] $Path) {
    $block = [regex]::Match((Get-Content -Raw -LiteralPath $Path), '(?ms)^targets\s*=\s*\[(.*?)^\]')
    if (-not $block.Success) { throw "no targets in $Path" }
    @([regex]::Matches($block.Groups[1].Value, '"([^"]+)"') | ForEach-Object { $_.Groups[1].Value } | Sort-Object)
}

function Escape-Html([string] $Text) { [System.Net.WebUtility]::HtmlEncode($Text) }

# Hai danh sách target lệch nhau ⇒ trang giấy phép không còn khớp thứ `cargo deny` đã soát.
$aboutTargets = Get-Targets (Join-Path $dir 'about.toml')
$denyTargets = Get-Targets (Join-Path $repo 'deny.toml')
if (($aboutTargets -join ',') -ne ($denyTargets -join ',')) { throw 'targets differ: about.toml vs deny.toml' }

Push-Location $repo
try {
    # `--frozen`: không gọi ClearlyDefined, chỉ đọc nguồn crate đã tải, chạy lại ra cùng nội dung.
    cargo fetch --locked
    if ($LASTEXITCODE -ne 0) { throw "cargo fetch failed (exit $LASTEXITCODE)" }
    cargo deny --log-level error check licenses
    if ($LASTEXITCODE -ne 0) { throw "cargo deny check licenses failed (exit $LASTEXITCODE)" }

    $raw = Join-Path ([IO.Path]::GetTempPath()) "groove-licenses-$([guid]::NewGuid().ToString('N')).html"
    try {
        cargo about generate --frozen --workspace --config (Join-Path $dir 'about.toml') --output-file $raw (Join-Path $dir 'template.hbs')
        if ($LASTEXITCODE -ne 0) { throw "cargo about failed (exit $LASTEXITCODE)" }
        $html = (Get-Content -Raw -Encoding utf8 -LiteralPath $raw) -replace "`r`n", "`n"
    } finally {
        Remove-Item -LiteralPath $raw -Force -ErrorAction SilentlyContinue
    }
} finally {
    Pop-Location
}

$articles = foreach ($c in $extra) {
    $file = (Resolve-Path (Join-Path $dir $c.Text)).Path
    $text = (Get-Content -Raw -Encoding utf8 -LiteralPath $file) -replace "`r`n", "`n"
    @(
        "<article data-license=`"$(Escape-Html $c.License)`" data-component=`"$(Escape-Html $c.Name)`">",
        "<h3>$(Escape-Html $c.Name) $(Escape-Html $c.Version): $(Escape-Html $c.License)</h3>",
        "<p class=`"note`"><a href=`"$(Escape-Html $c.Url)`">$(Escape-Html $c.Url)</a></p>",
        "<pre>$(Escape-Html $text)</pre>",
        '</article>'
    ) -join "`n"
}
$marker = '<!-- extra-components -->'
if (-not $html.Contains($marker)) { throw "template has no $marker" }
$html = $html.Replace($marker, ($articles -join "`n"))

# D-2: soát nhãn (không soát cả văn bản: MPL có nhắc tên GPL mà không áp dụng). Riêng crate thì nhãn LGPL nào cũng là lỗi.
$crateLabels = @([regex]::Matches($html, '<article data-license="([^"]+)">') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
$banned = @($crateLabels | Where-Object { $_ -match '(?i)\b(a|l)?gpl' })
if ($banned) { throw "banned license: $($banned -join ', ')" }
$componentLabels = @([regex]::Matches($html, 'data-license="([^"]+)" data-component="([^"]+)"') | ForEach-Object {
        [pscustomobject]@{ License = $_.Groups[1].Value; Name = [System.Net.WebUtility]::HtmlDecode($_.Groups[2].Value) }
    })
$bannedComponents = @($componentLabels | Where-Object { $_.License -match '(?i)\b(a|l)?gpl' -and $_.Name -ne $lgplAllowed })
if ($bannedComponents) { throw "banned license: $($bannedComponents.Name -join ', ')" }

# Thứ phải có: một target rơi khỏi cây thì tên của nó biến mất.
$required = 'gpui-pre', 'gpui-kit', 'cpal', 'alsa-sys', 'souvlaki', 'windows', 'objc2'
$crates = @([regex]::Matches($html, 'data-crate="([^"]+)"') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
$absent = @($required | Where-Object { $_ -notin $crates })
if ($absent) { throw "missing from the page: $($absent -join ', ')" }

New-Item -ItemType Directory -Force $Out | Out-Null
$file = Join-Path (Resolve-Path $Out).Path 'third-party.html'
[IO.File]::WriteAllText($file, $html, [Text.UTF8Encoding]::new($false))
"crates=$($crates.Count) licenses=$($crateLabels -join ', ')"
"sha256=$((Get-FileHash -Algorithm SHA256 -LiteralPath $file).Hash.ToLowerInvariant())"
"wrote $file"
