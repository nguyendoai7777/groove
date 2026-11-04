# Dựng icon tệp nhạc (`assets/app/groove-file.ico`) từ icon app (`assets/app/groove.ico`): cùng hình nhưng nhỏ hơn, có lề trong
# suốt quanh hình (~78 % ô) — Explorer đặt icon tệp trong ô 16 px ở chế độ Details, icon app lấp kín ô trông to hơn icon tệp khác.
#
#   pwsh tools/make-file-icon.ps1            # ghi đè assets/app/groove-file.ico
#   pwsh tools/make-file-icon.ps1 -Out x.ico
#
# Khung ≥ 32 px: thu khung 256 của icon app vào hộp giữa ô (giữ nét chữ G). Khung 16 / 20 / 24 px: vẽ lại bằng hình học canh lưới
# điểm ảnh (thu nhỏ khung 256 xuống 12 px thì nhoè): vành gradient cam → tím, đĩa đen, vòng trắng, lỗ giữa; tỉ lệ đo từ khung 256
# (đĩa 0,84 R, vòng trắng 0,38 R, lỗ 0,125 R); vành gradient cho dày hơn (đĩa 0,74 R) để ở 16 px còn ~1,5 px màu. Chữ G bỏ ở
# cỡ nhỏ (dưới 1 px). Mọi khung lưu PNG 32 bit trong `.ico`.
param(
    [string]$Source = (Join-Path $PSScriptRoot "../crates/groove-app/assets/app/groove.ico"),
    [string]$Out = (Join-Path $PSScriptRoot "../crates/groove-app/assets/app/groove-file.ico")
)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

# Cạnh ô → cạnh hình bên trong (chẵn để tâm rơi đúng giữa ô).
$Layout = [ordered]@{ 16 = 12; 20 = 16; 24 = 18; 32 = 26; 48 = 38; 64 = 50; 256 = 200 }
$VectorMax = 24

# Màu lấy mẫu từ khung 256 của icon app.
$RingFrom = [System.Drawing.Color]::FromArgb(255, 0xF9, 0x8F, 0x50)
$RingTo = [System.Drawing.Color]::FromArgb(255, 0x8F, 0x65, 0xBB)
$Disc = [System.Drawing.Color]::FromArgb(255, 0x36, 0x38, 0x37)
$Label = [System.Drawing.Color]::FromArgb(255, 0xEC, 0xEC, 0xEC)

function Read-Frames([string]$path) {
    $bytes = [System.IO.File]::ReadAllBytes($path)
    $count = [BitConverter]::ToUInt16($bytes, 4)
    $result = @{}
    for ($i = 0; $i -lt $count; $i++) {
        $o = 6 + $i * 16
        $w = if ($bytes[$o] -eq 0) { 256 } else { $bytes[$o] }
        $size = [BitConverter]::ToUInt32($bytes, $o + 8)
        $offset = [BitConverter]::ToUInt32($bytes, $o + 12)
        $result[[int]$w] = [byte[]]$bytes[$offset..($offset + $size - 1)]
    }
    $result
}

function New-Canvas([int]$edge) {
    $bitmap = New-Object System.Drawing.Bitmap $edge, $edge, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bitmap)
    $g.Clear([System.Drawing.Color]::Transparent)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $bitmap, $g
}

function Fill-Circle($g, $brush, [float]$c, [float]$r) {
    $g.FillEllipse($brush, $c - $r, $c - $r, 2 * $r, 2 * $r)
}

# Hình vẽ lại cho ô nhỏ: tâm giữa ô, bán kính ngoài = nửa cạnh hình.
function Draw-Small([int]$edge, [int]$inner) {
    $bitmap, $g = New-Canvas $edge
    $c = $edge / 2
    $r = $inner / 2
    $from = [System.Drawing.PointF]::new($c - $r, $c - $r)
    $to = [System.Drawing.PointF]::new($c + $r, $c + $r)
    $ring = [System.Drawing.Drawing2D.LinearGradientBrush]::new($from, $to, $RingFrom, $RingTo)
    # Chuyển màu chỉ ở khúc giữa đường chéo: hai đầu giữ cam / tím đặc như bản gốc, không pha thành hồng đều ở 16 px.
    $blend = [System.Drawing.Drawing2D.Blend]::new(4)
    $blend.Positions = [float[]]@(0, 0.3, 0.7, 1)
    $blend.Factors = [float[]]@(0, 0, 1, 1)
    $ring.Blend = $blend
    Fill-Circle $g $ring $c $r
    Fill-Circle $g (New-Object System.Drawing.SolidBrush $Disc) $c ($r * 0.74)
    Fill-Circle $g (New-Object System.Drawing.SolidBrush $Label) $c ($r * 0.40)
    Fill-Circle $g (New-Object System.Drawing.SolidBrush $Disc) $c ([Math]::Max($r * 0.13, 0.75))
    $g.Dispose()
    $bitmap
}

# Thu khung lớn của icon app vào hộp giữa ô.
function Draw-Scaled([System.Drawing.Image]$source, [int]$edge, [int]$inner) {
    $bitmap, $g = New-Canvas $edge
    $offset = ($edge - $inner) / 2
    $g.DrawImage($source, (New-Object System.Drawing.Rectangle $offset, $offset, $inner, $inner))
    $g.Dispose()
    $bitmap
}

function Get-Png([System.Drawing.Bitmap]$bitmap) {
    $stream = New-Object System.IO.MemoryStream
    $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
    , $stream.ToArray()
}

$sourceFrames = Read-Frames $Source
if (-not $sourceFrames.ContainsKey(256)) { throw "$Source thiếu khung 256" }
$big = [System.Drawing.Image]::FromStream((New-Object System.IO.MemoryStream (, $sourceFrames[256])))

$images = foreach ($entry in $Layout.GetEnumerator()) {
    $edge = [int]$entry.Key
    $inner = [int]$entry.Value
    $bitmap = if ($edge -le $VectorMax) { Draw-Small $edge $inner } else { Draw-Scaled $big $edge $inner }
    $png = Get-Png $bitmap
    $bitmap.Dispose()
    [pscustomobject]@{ Edge = $edge; Png = $png }
}

# ICONDIR + ICONDIRENTRY (16 byte mỗi khung) + dữ liệu PNG.
$ms = New-Object System.IO.MemoryStream
$w = New-Object System.IO.BinaryWriter $ms
$w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$images.Count)
$offset = 6 + 16 * $images.Count
foreach ($image in $images) {
    $byte = if ($image.Edge -ge 256) { 0 } else { $image.Edge }
    $w.Write([byte]$byte); $w.Write([byte]$byte); $w.Write([byte]0); $w.Write([byte]0)
    $w.Write([uint16]1); $w.Write([uint16]32)
    $w.Write([uint32]$image.Png.Length); $w.Write([uint32]$offset)
    $offset += $image.Png.Length
}
foreach ($image in $images) { $w.Write($image.Png) }
$w.Flush()
[System.IO.File]::WriteAllBytes((Resolve-Path -LiteralPath (Split-Path $Out -Parent)).Path + "\" + (Split-Path $Out -Leaf), $ms.ToArray())
Write-Host "wrote $Out ($($images.Count) frames: $(($images | ForEach-Object Edge) -join ', '))"
