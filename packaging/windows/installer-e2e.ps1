<#
.SYNOPSIS
  Soát bản cài thật, không mở app (groove-gd1_27, gd2_01): cài im lặng vào thư mục tạm → tệp, icon + version info của exe, lối tắt
  Start Menu + AppUserModelID, ProgID, `OpenWithProgids`, `Capabilities`, `RegisteredApplications`, mục gỡ → Windows thấy app trong
  "Mở bằng" và Default apps (chỉ hỏi, không đặt) → exe bị giữ thì cài đè, gỡ dừng mã 2 → gỡ im lặng → registry, lối tắt về như cũ.

  Ghi HKCU của máy đang chạy rồi dọn. Không đụng `UserChoice`, không đổi liên kết mặc định, không mở Settings, không gọi hàm đặt
  mặc định nào của Windows. Máy đã cài Groove X (khoá, ProgID, lối tắt có sẵn) thì từ chối, không đụng bản đang dùng.

.EXAMPLE
  pwsh packaging/windows/installer-e2e.ps1 -Setup dist/groove-x-0.2.0-setup.exe
#>
param([Parameter(Mandatory)] [string] $Setup)
$ErrorActionPreference = 'Stop'
$Setup = (Resolve-Path $Setup).Path

$classes = 'HKCU:\Software\Classes'
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\GrooveX'
$clientKey = 'HKCU:\Software\Clients\Media\Groove X'
$capabilities = "$clientKey\Capabilities"
$registeredApps = 'HKCU:\Software\RegisteredApplications'
$appName = 'Groove X'
$aumid = 'GrooveX.Player'
# Đuôi, tên loại tệp: khớp `AUDIO_EXTENSIONS` và `groove.nsi` (test `packaging.rs` soát hai chỗ kia).
$extensions = [ordered]@{ mp3 = 'MP3'; flac = 'FLAC'; wav = 'WAV'; m4a = 'M4A'; m4r = 'M4R'; ogg = 'OGG' }
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Groove X.lnk'
$desktopShortcut = Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) 'Groove X.lnk'

function Test-Value([string] $Key, [string] $Name) {
    (Test-Path $Key) -and ((Get-Item $Key).Property -contains $Name)
}
function Get-Value([string] $Key, [string] $Name) {
    if (Test-Path $Key) { (Get-Item $Key).GetValue($Name) } else { $null }
}

$present = @(
    (Test-Path $uninstallKey), (Test-Path $clientKey), (Test-Value $registeredApps $appName), (Test-Path $shortcut),
    (Test-Path $desktopShortcut), (Test-Path "$classes\Applications\groove.exe"), (Test-Path "$classes\GrooveX.AudioFile")
) + @($extensions.Keys | ForEach-Object { Test-Path "$classes\GrooveX.$_" })
if ($present -contains $true) { throw 'Groove X is installed on this machine: not touching it' }

function Check([string] $Name, [bool] $Ok) {
    Write-Host ("{0,-4} {1}" -f $(if ($Ok) { 'ok' } else { 'FAIL' }), $Name)
    if (-not $Ok) { $script:failed = $true }
}

# Mọi thứ của máy mà bộ cài có thể chạm, kể cả `UserChoice` (chỉ đọc): phải y như cũ sau khi gỡ.
function Snapshot {
    foreach ($ext in $extensions.Keys) {
        $key = "$classes\.$ext"
        $open = if (Test-Path "$key\OpenWithProgids") { ((Get-Item "$key\OpenWithProgids").Property | Sort-Object) -join ',' } else { '-' }
        $default = if (Test-Path $key) { (Get-Item $key).GetValue('') } else { '-' }
        $choice = Get-Value "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.$ext\UserChoice" 'ProgId'
        "$ext exists=$(Test-Path $key) default=$default choice=$choice open=$open"
    }
    "registered=$(((Get-Item $registeredApps -ErrorAction Ignore).Property | Sort-Object) -join ',')"
    "clients=$(Test-Path 'HKCU:\Software\Clients') media=$(Test-Path 'HKCU:\Software\Clients\Media')"
    "progids=$((Get-ChildItem $classes -ErrorAction Ignore | Where-Object PSChildName -Like 'GrooveX*' | ForEach-Object PSChildName) -join ',')"
}
function Run([string] $Exe, [string[]] $Arguments) {
    (Start-Process -FilePath $Exe -ArgumentList $Arguments -Wait -PassThru).ExitCode
}

# Hỏi Windows (chỉ đọc): danh sách "Mở bằng" của một đuôi, app có được Default apps nhận không, và tên hiện của ProgID.
Add-Type -Namespace GrooveE2e -Name Native -MemberDefinition @'
[DllImport("shell32.dll", CharSet = CharSet.Unicode)]
public static extern uint ExtractIconExW(string file, int index, IntPtr large, IntPtr small, uint count);
[DllImport("shlwapi.dll", CharSet = CharSet.Unicode)]
public static extern int AssocQueryStringW(int flags, int str, string assoc, string extra, System.Text.StringBuilder outText, ref uint size);
'@
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
namespace GrooveE2e {
  [ComImport, Guid("F04061AC-1659-4a3f-A954-775AA57FC083"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  public interface IAssocHandler {
    [PreserveSig] int GetName([MarshalAs(UnmanagedType.LPWStr)] out string name);
    [PreserveSig] int GetUIName([MarshalAs(UnmanagedType.LPWStr)] out string name);
  }
  [ComImport, Guid("973810ae-9599-4b88-9e4d-6ee98c9552da"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  public interface IEnumAssocHandlers {
    [PreserveSig] int Next(uint count, [Out, MarshalAs(UnmanagedType.LPArray, ArraySubType = UnmanagedType.Interface, SizeParamIndex = 0)] IAssocHandler[] items, out uint fetched);
  }
  // Chỉ khai ba hàm hỏi đầu bảng; hàm đặt mặc định đứng sau, không khai nên không gọi được.
  [ComImport, Guid("4e530b0a-e611-4c77-a3ac-9031d022281b"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  public interface IApplicationAssociationRegistration {
    [PreserveSig] int QueryCurrentDefault([MarshalAs(UnmanagedType.LPWStr)] string query, int type, int level, [MarshalAs(UnmanagedType.LPWStr)] out string assoc);
    [PreserveSig] int QueryAppIsDefault([MarshalAs(UnmanagedType.LPWStr)] string query, int type, int level, [MarshalAs(UnmanagedType.LPWStr)] string app, [MarshalAs(UnmanagedType.Bool)] out bool isDefault);
  }
  public static class Assoc {
    [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
    static extern int SHAssocEnumHandlers(string extension, int filter, out IEnumAssocHandlers handlers);

    /// Đường dẫn exe của từng mục trong "Mở bằng" (ASSOC_FILTER_NONE).
    public static List<string> OpenWith(string extension) {
      var found = new List<string>();
      IEnumAssocHandlers handlers;
      if (SHAssocEnumHandlers(extension, 0, out handlers) != 0 || handlers == null) return found;
      var one = new IAssocHandler[1];
      uint fetched;
      while (handlers.Next(1, one, out fetched) == 0 && fetched == 1) {
        string name;
        if (one[0].GetName(out name) == 0) found.Add(name);
        Marshal.ReleaseComObject(one[0]);
      }
      Marshal.ReleaseComObject(handlers);
      return found;
    }

    /// HRESULT của QueryAppIsDefault(AT_FILEEXTENSION, AL_EFFECTIVE): S_OK nghĩa là Windows biết app đã đăng ký.
    public static int Registered(string extension, string app) {
      var type = Type.GetTypeFromCLSID(new Guid("591209c7-767b-42b2-9fba-44ee4615f2c7"));
      var reg = (IApplicationAssociationRegistration)Activator.CreateInstance(type);
      bool isDefault;
      int hr = reg.QueryAppIsDefault(extension, 0, 1, app, out isDefault);
      Marshal.ReleaseComObject(reg);
      return hr;
    }
  }
}
'@
function Get-Assoc([string] $Assoc, [int] $What, [int] $Flags = 0x400) {
    # 0x400 ASSOCF_INIT_IGNOREUNKNOWN, 0x2 ASSOCF_INIT_BYEXENAME; 3 = ASSOCSTR_FRIENDLYDOCNAME, 4 = ASSOCSTR_FRIENDLYAPPNAME.
    $text = [Text.StringBuilder]::new(512)
    [uint32] $size = 512
    if ([GrooveE2e.Native]::AssocQueryStringW($Flags, $What, $Assoc, $null, $text, [ref] $size) -eq 0) { $text.ToString() } else { $null }
}
function Get-ShortcutAumid([string] $Path) {
    $folder = (New-Object -ComObject Shell.Application).Namespace((Split-Path -Parent $Path))
    $folder.ParseName((Split-Path -Leaf $Path)).ExtendedProperty('System.AppUserModel.ID')
}

$before = Snapshot
$dir = Join-Path ([IO.Path]::GetTempPath()) "groove-e2e-$([guid]::NewGuid().ToString('N'))"
$installed = Join-Path $dir 'Groove X'
$failed = $false
try {
    # `/D=` phải đứng cuối, không ngoặc kép, được phép có dấu cách.
    Check 'install exit 0' ((Run $Setup @('/S', "/D=$installed")) -eq 0)
    $exe = Join-Path $installed 'groove.exe'
    $command = "`"$exe`" `"%1`""
    Check 'groove.exe' (Test-Path $exe)
    Check 'uninstall.exe' (Test-Path (Join-Path $installed 'uninstall.exe'))
    Check 'third-party.html' (Test-Path (Join-Path $installed 'licenses/third-party.html'))
    $page = Get-Content -Raw -Encoding utf8 (Join-Path $installed 'licenses/third-party.html')
    Check 'page names libasound' ($page.Contains('data-component="ALSA library (libasound)"'))
    Check 'no libasound file' (-not (Get-ChildItem $installed -Recurse -Filter 'libasound*'))

    Check 'exe has an icon' ([GrooveE2e.Native]::ExtractIconExW($exe, -1, [IntPtr]::Zero, [IntPtr]::Zero, 0) -ge 1)
    $info = (Get-Item $exe).VersionInfo
    Check 'exe ProductName, FileDescription, CompanyName' (
        $info.ProductName -eq $appName -and $info.FileDescription -eq $appName -and $info.CompanyName -eq $appName)
    Check 'exe version matches the setup' ($info.ProductVersion -and $Setup.Contains("-$($info.ProductVersion)-setup"))

    Check 'Start Menu shortcut' (Test-Path $shortcut)
    $link = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcut)
    Check 'shortcut target' ($link.TargetPath -eq $exe)
    Check "shortcut AppUserModelID $aumid" ((Get-ShortcutAumid $shortcut) -eq $aumid)
    Check 'no Desktop shortcut by default' (-not (Test-Path $desktopShortcut))

    Check 'Applications\groove.exe name' ((Get-Value "$classes\Applications\groove.exe" 'FriendlyAppName') -eq $appName)
    Check 'Windows reads the app name' ((Get-Assoc $exe 4 0x402) -eq $appName)
    Check 'Capabilities name, description, icon' (
        (Get-Value $capabilities 'ApplicationName') -eq $appName -and (Get-Value $capabilities 'ApplicationDescription') -and
        (Get-Value $capabilities 'ApplicationIcon') -eq "$exe,0")
    Check 'RegisteredApplications' ((Get-Value $registeredApps $appName) -eq 'Software\Clients\Media\Groove X\Capabilities')
    foreach ($ext in $extensions.Keys) {
        $progId = "GrooveX.$ext"
        $key = "$classes\$progId"
        Check "$progId open command" ((Get-Value "$key\shell\open\command" '') -eq $command)
        Check "$progId icon, AppUserModelID" ((Get-Value "$key\DefaultIcon" '') -eq "$exe,0" -and (Get-Value $key 'AppUserModelID') -eq $aumid)
        Check "$progId type name" ((Get-Assoc $progId 3) -like "*$($extensions[$ext])*")
        Check ".$ext in OpenWithProgids" (Test-Value "$classes\.$ext\OpenWithProgids" $progId)
        Check ".$ext in Capabilities" ((Get-Value "$capabilities\FileAssociations" ".$ext") -eq $progId)
        Check ".$ext offers Groove X in Open with" ([GrooveE2e.Assoc]::OpenWith(".$ext") -contains $exe)
        Check ".$ext Default apps knows Groove X" ([GrooveE2e.Assoc]::Registered(".$ext", $appName) -eq 0)
    }
    Check 'uninstall entry: icon, version, publisher, size' (
        (Get-Value $uninstallKey 'DisplayIcon') -eq "$exe,0" -and (Get-Value $uninstallKey 'DisplayVersion') -eq $info.ProductVersion -and
        (Get-Value $uninstallKey 'Publisher') -eq $appName -and (Get-Value $uninstallKey 'EstimatedSize') -gt 0)
    # Mặc định, `UserChoice` y như trước: bộ cài không cướp liên kết.
    $keep = { param($lines) @($lines | Where-Object { $_ -notmatch '^(registered|clients|progids)=' } | ForEach-Object { ($_ -split ' open=')[0] }) }
    Check 'default association untouched' (((& $keep $before) -join ';') -eq ((& $keep (Snapshot)) -join ';'))

    $hold = [IO.File]::Open($exe, 'Open', 'Read', 'Read')
    try {
        Check 'upgrade over a locked exe stops with 2' ((Run $Setup @('/S', "/D=$installed")) -eq 2)
        Check 'uninstall with a locked exe stops with 2' ((Run (Join-Path $installed 'uninstall.exe') @('/S', "_?=$installed")) -eq 2)
        Check 'still registered' (Test-Path $uninstallKey)
    } finally {
        $hold.Dispose()
    }
    Check 'upgrade after closing exit 0' ((Run $Setup @('/S', "/D=$installed")) -eq 0)
    Check 'upgrade keeps the shortcut AppUserModelID' ((Get-ShortcutAumid $shortcut) -eq $aumid)

    # `_?=`: chạy tại chỗ, không chép sang thư mục tạm, nên chờ được nó xong. Gỡ im lặng giữ dữ liệu người dùng (không tạo ở đây).
    Check 'uninstall exit 0' ((Run (Join-Path $installed 'uninstall.exe') @('/S', "_?=$installed")) -eq 0)
    Check 'files gone' (-not (Test-Path $exe) -and -not (Test-Path (Join-Path $installed 'licenses')))
    Check 'shortcut gone' (-not (Test-Path $shortcut))
    Check 'uninstall key gone' (-not (Test-Path $uninstallKey))
    Check 'ProgIDs gone' (-not ($extensions.Keys | Where-Object { Test-Path "$classes\GrooveX.$_" }))
    Check 'Applications key gone' (-not (Test-Path "$classes\Applications\groove.exe"))
    Check 'Capabilities gone' (-not (Test-Path $clientKey))
    Check 'RegisteredApplications value gone' (-not (Test-Value $registeredApps $appName))
    Check '.mp3 Open with no longer offers it' (-not ([GrooveE2e.Assoc]::OpenWith('.mp3') -contains $exe))
    $after = Snapshot
    Check 'registry as before' (($before -join ';') -eq ($after -join ';'))
    if (($before -join ';') -ne ($after -join ';')) { Compare-Object $before $after | Format-Table -AutoSize | Out-String | Write-Host }
} finally {
    # Dừng giữa chừng: gỡ bằng chính bộ gỡ, rồi xoá nốt thứ còn sót. Mọi khoá dưới đây đã soát là chưa có trước khi chạy.
    $uninstaller = Join-Path $installed 'uninstall.exe'
    # `_?=` không cho bộ gỡ tự xoá mình: còn `uninstall.exe` chưa chắc là chưa gỡ, xem khoá gỡ.
    if ((Test-Path $uninstaller) -and (Test-Path $uninstallKey)) {
        Write-Host 'cleanup: running the uninstaller'
        Run $uninstaller @('/S', "_?=$installed") | Out-Null
    }
    $leftovers = @($uninstallKey, $clientKey, "$classes\Applications\groove.exe", "$classes\GrooveX.AudioFile") +
        @($extensions.Keys | ForEach-Object { "$classes\GrooveX.$_" })
    foreach ($key in $leftovers) {
        if (Test-Path $key) { Write-Host "cleanup: $key"; Remove-Item -Recurse -Force $key; $failed = $true }
    }
    if (Test-Value $registeredApps $appName) { Remove-ItemProperty $registeredApps $appName; $failed = $true }
    foreach ($ext in $extensions.Keys) {
        if (Test-Value "$classes\.$ext\OpenWithProgids" "GrooveX.$ext") { Remove-ItemProperty "$classes\.$ext\OpenWithProgids" "GrooveX.$ext"; $failed = $true }
    }
    foreach ($link in $shortcut, $desktopShortcut) { if (Test-Path $link) { Remove-Item -Force $link; $failed = $true } }
    Remove-Item -Recurse -Force -LiteralPath $dir -ErrorAction SilentlyContinue
}
if ($failed) { exit 1 }
Write-Host 'all ok'
