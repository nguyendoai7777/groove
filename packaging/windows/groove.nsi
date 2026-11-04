# Bản cài Windows của Groovex (groove-gd1_27, groove-gd2_01). Dựng bằng `packaging/windows/build-installer.ps1`, không gọi tay:
# script truyền VERSION, VERSION4, STAGE, ICON, OUTFILE qua /D và đọc tệp này với /INPUTCHARSET UTF8.
#
# Cài theo người dùng, không cần admin, không ký (ký là dịch vụ trả phí; SmartScreen sẽ cảnh báo). Chỉ ghi HKCU.
#
# Đăng ký như một trình phát chính thức (đặc tả 5.6, gd2_01):
# - Mỗi đuôi một ProgID `GrooveX.<đuôi>` (tên loại tệp đọc được, icon, lệnh mở) và giá trị trong `.<đuôi>\OpenWithProgids`: hiện
#   trong "Mở bằng".
# - `Capabilities` (tên, mô tả, icon, `FileAssociations`) + `RegisteredApplications`: hiện trong Settings → Default apps, chọn được
#   cho từng đuôi và ở ô Trình phát nhạc.
# - KHÔNG ghi giá trị mặc định của `.<đuôi>`, KHÔNG đụng `UserChoice`: Windows không cho tự đặt mặc định, người dùng tự chọn. Trang
#   cuối có ô mở đúng trang Default apps.
# Danh sách đuôi phải khớp `AUDIO_EXTENSIONS`: `crates/groove-app/tests/packaging.rs` soát.
#
# Lối tắt mang AppUserModelID `${AUMID}`, trùng `platform::default_player` đặt cho tiến trình: taskbar gom cửa sổ với lối tắt đã
# ghim, media session hiện tên và icon của lối tắt.
#
# Phiên đang chạy: không giết theo tên, máy dev có thể đang chạy bản khác. Exe trong thư mục cài còn bị giữ thì hỏi đóng rồi thử lại;
# cài im lặng thì dừng mã 2, không ghi đè nửa chừng. Cài đè giữ dữ liệu (dữ liệu không nằm trong thư mục cài). Gỡ cài hỏi có xoá
# thư viện, cài đặt không, mặc định giữ; chỉ xoá thứ bộ cài đã đặt.

Unicode true
SetCompressor /SOLID lzma
ManifestDPIAware true
RequestExecutionLevel user

!include "MUI2.nsh"
!include "Sections.nsh"

!define APP_NAME "Groovex"
; D-21: nhà phát hành, trang chủ chung mọi app (`crates/groove-app/build.rs` cùng giá trị).
!define PUBLISHER "z0 team, sandtechs jsc."
!define HOMEPAGE "https://sandtechs.com"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\GrooveX"
!define UNINST_EXE "uninstall.exe"
!define CLASSES "Software\Classes"
!define PROG_PREFIX "GrooveX."
# ProgID chung của bản gd1_27; cài đè thì dọn.
!define OLD_PROG_ID "GrooveX.AudioFile"
!define CLIENT_KEY "Software\Clients\Media\Groovex"
!define CAPABILITIES "${CLIENT_KEY}\Capabilities"
!define REGISTERED_APPS "Software\RegisteredApplications"
!define AUMID "Sandtechs.Groovex"
!define DEFAULT_APPS_URI "ms-settings:defaultapps?registeredAppUser=Groovex"
!define START_LINK "$SMPROGRAMS\Groovex.lnk"
!define DESKTOP_LINK "$DESKTOP\Groovex.lnk"
# 250 ms mỗi lượt: chờ tối đa 5 s.
!define LOCK_ROUNDS 20

Name "${APP_NAME}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\Groovex"
InstallDirRegKey HKCU "${UNINST_KEY}" "InstallLocation"
BrandingText "Groovex ${VERSION}"

VIProductVersion "${VERSION4}"
VIAddVersionKey /LANG=0 "ProductName" "Groovex"
VIAddVersionKey /LANG=0 "CompanyName" "${PUBLISHER}"
VIAddVersionKey /LANG=0 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=0 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=0 "FileDescription" "Groovex setup"
VIAddVersionKey /LANG=0 "LegalCopyright" "Copyright © 2026 Sandtechs"

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_ABORTWARNING
!define MUI_COMPONENTSPAGE_NODESC
!define MUI_FINISHPAGE_RUN "$INSTDIR\groove.exe"
!define MUI_FINISHPAGE_RUN_TEXT "$(run_groove)"
# Ô thứ hai của trang cuối: mở trang Default apps, người dùng tự chọn.
!define MUI_FINISHPAGE_SHOWREADME ""
!define MUI_FINISHPAGE_SHOWREADME_TEXT "$(make_default)"
!define MUI_FINISHPAGE_SHOWREADME_FUNCTION OpenDefaultApps

!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

# Ngôn ngữ đầu là mặc định khi máy không dùng ngôn ngữ nào trong danh sách.
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Vietnamese"

LangString run_groove ${LANG_ENGLISH} "Open Groovex"
LangString run_groove ${LANG_VIETNAMESE} "Mở Groovex"
LangString make_default ${LANG_ENGLISH} "Make Groovex the default music player"
LangString make_default ${LANG_VIETNAMESE} "Đặt Groovex làm trình phát nhạc mặc định"
LangString still_running ${LANG_ENGLISH} "Groovex is still running. Close it, then choose Retry."
LangString still_running ${LANG_VIETNAMESE} "Groovex vẫn đang chạy. Hãy đóng nó rồi chọn Retry."
LangString remove_data ${LANG_ENGLISH} "Also delete your Groovex library and settings?$\n$\nYour music files are never deleted. Choose No to keep them for a later install."
LangString remove_data ${LANG_VIETNAMESE} "Xoá luôn thư viện và cài đặt của Groovex?$\n$\nTệp nhạc của bạn không bao giờ bị xoá. Chọn No để giữ lại cho lần cài sau."
LangString app_description ${LANG_ENGLISH} "Music player, a replacement for Microsoft's Groove Music"
LangString app_description ${LANG_VIETNAMESE} "Ứng dụng nghe nhạc thay thế cho Groove của Microsoft"
LangString sec_app ${LANG_ENGLISH} "Groovex (required)"
LangString sec_app ${LANG_VIETNAMESE} "Groovex (bắt buộc)"
LangString sec_desktop ${LANG_ENGLISH} "Desktop shortcut"
LangString sec_desktop ${LANG_VIETNAMESE} "Lối tắt trên Desktop"
# Tên loại tệp: "<tên> (MP3)". NSIS coi LangString rỗng là chưa đặt, nên không ghép trước / sau theo ngôn ngữ.
LangString type_name ${LANG_ENGLISH} "Audio file"
LangString type_name ${LANG_VIETNAMESE} "Tệp nhạc"

!macro StopAppFunction PREFIX
Function ${PREFIX}StopApp
  IfFileExists "$INSTDIR\groove.exe" 0 done
  StrCpy $R0 0
  poll:
    # FileOpen chế độ "a" báo lỗi khi exe đang chạy; tệp đã soát là có nên không tạo mới.
    ClearErrors
    FileOpen $R2 "$INSTDIR\groove.exe" a
    IfErrors locked
    FileClose $R2
    Goto done
  locked:
    IntOp $R0 $R0 + 1
    IntCmp $R0 ${LOCK_ROUNDS} ask
    Sleep 250
    Goto poll
  ask:
    MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "$(still_running)" /SD IDCANCEL IDRETRY retry
    SetErrorLevel 2
    Quit
  retry:
    StrCpy $R0 0
    Goto poll
  done:
FunctionEnd
!macroend

!insertmacro StopAppFunction ""
!insertmacro StopAppFunction "un."

# EXT: đuôi chữ thường, như `AUDIO_EXTENSIONS`; LABEL: tên hiện trong loại tệp.
!macro AssociateExtension EXT LABEL
  WriteRegStr HKCU "${CLASSES}\${PROG_PREFIX}${EXT}" "" "$(type_name) (${LABEL})"
  WriteRegStr HKCU "${CLASSES}\${PROG_PREFIX}${EXT}" "AppUserModelID" "${AUMID}"
  # Icon tệp riêng (resource thứ hai của exe): cùng hình icon app nhưng nhỏ hơn, có lề — build.rs, tools/make-file-icon.ps1.
  WriteRegStr HKCU "${CLASSES}\${PROG_PREFIX}${EXT}\DefaultIcon" "" "$INSTDIR\groove.exe,1"
  # Không giới hạn 15 tệp khi chọn nhiều: mỗi tệp một tiến trình, `open_paths` gộp lại.
  WriteRegStr HKCU "${CLASSES}\${PROG_PREFIX}${EXT}\shell\open" "MultiSelectModel" "Player"
  WriteRegStr HKCU "${CLASSES}\${PROG_PREFIX}${EXT}\shell\open\command" "" '"$INSTDIR\groove.exe" "%1"'
  WriteRegStr HKCU "${CLASSES}\.${EXT}\OpenWithProgids" "${PROG_PREFIX}${EXT}" ""
  WriteRegStr HKCU "${CLASSES}\Applications\groove.exe\SupportedTypes" ".${EXT}" ""
  WriteRegStr HKCU "${CAPABILITIES}\FileAssociations" ".${EXT}" "${PROG_PREFIX}${EXT}"
  DeleteRegValue HKCU "${CLASSES}\.${EXT}\OpenWithProgids" "${OLD_PROG_ID}"
!macroend

!macro ForgetExtension EXT
  DeleteRegValue HKCU "${CLASSES}\.${EXT}\OpenWithProgids" "${PROG_PREFIX}${EXT}"
  DeleteRegValue HKCU "${CLASSES}\.${EXT}\OpenWithProgids" "${OLD_PROG_ID}"
  DeleteRegKey HKCU "${CLASSES}\${PROG_PREFIX}${EXT}"
  # Khoá rỗng mới xoá: ứng dụng khác có thể đang đứng cùng chỗ.
  DeleteRegKey /ifempty HKCU "${CLASSES}\.${EXT}\OpenWithProgids"
  DeleteRegKey /ifempty HKCU "${CLASSES}\.${EXT}"
!macroend

# Thêm, bớt đuôi thì sửa cả hai danh sách; test so với `AUDIO_EXTENSIONS`.
!macro AssociateAll
  !insertmacro AssociateExtension "mp3" "MP3"
  !insertmacro AssociateExtension "flac" "FLAC"
  !insertmacro AssociateExtension "wav" "WAV"
  !insertmacro AssociateExtension "m4a" "M4A"
  !insertmacro AssociateExtension "m4r" "M4R"
  !insertmacro AssociateExtension "ogg" "OGG"
!macroend

!macro ForgetAll
  !insertmacro ForgetExtension "mp3"
  !insertmacro ForgetExtension "flac"
  !insertmacro ForgetExtension "wav"
  !insertmacro ForgetExtension "m4a"
  !insertmacro ForgetExtension "m4r"
  !insertmacro ForgetExtension "ogg"
!macroend

# SHCNE_ASSOCCHANGED: Explorer đọc lại liên kết, không phải khởi động lại.
!macro NotifyShell
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

# Gắn System.AppUserModel.ID vào lối tắt `$R9` (NSIS không có lệnh sẵn): IShellLinkW → IPersistFile::Load → IPropertyStore::SetValue
# → Commit → IPersistFile::Save. Hỏng bước nào thì lối tắt vẫn dùng được, chỉ không gom taskbar: ghi chi tiết, không dừng cài.
Function SetShortcutAumid
  Push $R0
  Push $R1
  Push $R2
  Push $R3
  Push $R4
  Push $R5
  Push $R6
  StrCpy $R1 0
  StrCpy $R2 0
  StrCpy $R3 0
  System::Call 'ole32::CoInitialize(p 0)'
  # CLSID_ShellLink, IID_IShellLinkW, CLSCTX_INPROC_SERVER.
  System::Call 'ole32::CoCreateInstance(g "{00021401-0000-0000-C000-000000000046}", p 0, i 1, g "{000214F9-0000-0000-C000-000000000046}", *p .R1) i .R0'
  IntCmp $R0 0 0 fail fail
  # IUnknown::QueryInterface(IID_IPersistFile).
  System::Call '$R1->0(g "{0000010B-0000-0000-C000-000000000046}", *p .R2) i .R0'
  IntCmp $R0 0 0 fail fail
  # IPersistFile::Load(path, STGM_READWRITE).
  System::Call '$R2->5(w R9, i 2) i .R0'
  IntCmp $R0 0 0 fail fail
  # QueryInterface(IID_IPropertyStore).
  System::Call '$R1->0(g "{886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99}", *p .R3) i .R0'
  IntCmp $R0 0 0 fail fail
  # PKEY_AppUserModel_ID = {9F4C2855-…}, 5; PROPVARIANT VT_LPWSTR (31) trỏ tới chuỗi.
  System::Call '*(&g16 "{9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}", i 5) p .R4'
  System::Call '*(&w64 "${AUMID}") p .R6'
  System::Call '*(&i2 31, &i2 0, &i2 0, &i2 0, p R6, p 0) p .R5'
  # IPropertyStore::SetValue (6), Commit (7).
  System::Call '$R3->6(p R4, p R5) i .R0'
  System::Free $R4
  System::Free $R5
  System::Free $R6
  IntCmp $R0 0 0 fail fail
  System::Call '$R3->7() i .R0'
  IntCmp $R0 0 0 fail fail
  # IPersistFile::Save(NULL, TRUE): ghi vào chính tệp đã nạp.
  System::Call '$R2->6(p 0, i 1) i .R0'
  IntCmp $R0 0 done fail fail
  fail:
    IntFmt $R0 "0x%08X" $R0
    DetailPrint "AppUserModelID: $R9 ($R0)"
  done:
  IntCmp $R3 0 +2
    System::Call '$R3->2()'
  IntCmp $R2 0 +2
    System::Call '$R2->2()'
  IntCmp $R1 0 +2
    System::Call '$R1->2()'
  Pop $R6
  Pop $R5
  Pop $R4
  Pop $R3
  Pop $R2
  Pop $R1
  Pop $R0
FunctionEnd

Function OpenDefaultApps
  ExecShell "open" "${DEFAULT_APPS_URI}"
FunctionEnd

Section "$(sec_app)" SecMain
  SectionIn RO
  Call StopApp

  SetOutPath "$INSTDIR"
  File "${STAGE}\groove.exe"

  # `uninstall.exe` chứng tỏ thư mục này do bộ cài đặt: `/D=` trỏ nhầm chỗ có sẵn `licenses/` của người dùng thì không xoá.
  IfFileExists "$INSTDIR\${UNINST_EXE}" 0 +2
    RMDir /r "$INSTDIR\licenses"
  SetOutPath "$INSTDIR\licenses"
  File /r "${STAGE}\licenses\*"

  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\${UNINST_EXE}"
  CreateShortcut "${START_LINK}" "$INSTDIR\groove.exe" "" "$INSTDIR\groove.exe" 0 SW_SHOWNORMAL "" "$(app_description)"
  StrCpy $R9 "${START_LINK}"
  Call SetShortcutAumid

  # Bản gd1_27 dùng một ProgID chung.
  DeleteRegKey HKCU "${CLASSES}\${OLD_PROG_ID}"

  WriteRegStr HKCU "${CLASSES}\Applications\groove.exe" "FriendlyAppName" "Groovex"
  WriteRegStr HKCU "${CLASSES}\Applications\groove.exe\DefaultIcon" "" "$INSTDIR\groove.exe,0"
  WriteRegStr HKCU "${CLASSES}\Applications\groove.exe\shell\open\command" "" '"$INSTDIR\groove.exe" "%1"'
  WriteRegStr HKCU "${CAPABILITIES}" "ApplicationName" "Groovex"
  WriteRegStr HKCU "${CAPABILITIES}" "ApplicationDescription" "$(app_description)"
  WriteRegStr HKCU "${CAPABILITIES}" "ApplicationIcon" "$INSTDIR\groove.exe,0"
  !insertmacro AssociateAll
  WriteRegStr HKCU "${REGISTERED_APPS}" "${APP_NAME}" "${CAPABILITIES}"
  !insertmacro NotifyShell

  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "Groovex"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINST_KEY}" "URLInfoAbout" "${HOMEPAGE}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\groove.exe,0"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\${UNINST_EXE}"'
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\${UNINST_EXE}" /S'
  WriteRegStr HKCU "${UNINST_KEY}" "Comments" "$(app_description)"
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  SectionGetSize ${SecMain} $0
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" $0
SectionEnd

Section /o "$(sec_desktop)" SecDesktop
  CreateShortcut "${DESKTOP_LINK}" "$INSTDIR\groove.exe" "" "$INSTDIR\groove.exe" 0 SW_SHOWNORMAL "" "$(app_description)"
  StrCpy $R9 "${DESKTOP_LINK}"
  Call SetShortcutAumid
SectionEnd

# Cài đè: giữ lựa chọn lối tắt Desktop của lần trước.
Function .onInit
  IfFileExists "${DESKTOP_LINK}" 0 +2
    SectionSetFlags ${SecDesktop} ${SF_SELECTED}
FunctionEnd

Section "Uninstall"
  Call un.StopApp

  !insertmacro ForgetAll
  DeleteRegKey HKCU "${CLASSES}\${OLD_PROG_ID}"
  DeleteRegKey HKCU "${CLASSES}\Applications\groove.exe"
  DeleteRegValue HKCU "${REGISTERED_APPS}" "${APP_NAME}"
  DeleteRegKey HKCU "${CLIENT_KEY}"
  DeleteRegKey /ifempty HKCU "Software\Clients\Media"
  DeleteRegKey /ifempty HKCU "Software\Clients"
  !insertmacro NotifyShell

  Delete "${START_LINK}"
  Delete "${DESKTOP_LINK}"
  DeleteRegKey HKCU "${UNINST_KEY}"

  Delete "$INSTDIR\groove.exe"
  RMDir /r "$INSTDIR\licenses"
  Delete "$INSTDIR\${UNINST_EXE}"
  # Không /r: thư mục còn tệp lạ thì để nguyên.
  RMDir "$INSTDIR"

  # Dữ liệu của người dùng (`groove_core::paths`): `data_dir` ở Roaming, `cache_dir` ở Local. Gỡ im lặng thì giữ.
  MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "$(remove_data)" /SD IDNO IDNO keep_data
    RMDir /r "$APPDATA\Groove X"
    RMDir /r "$LOCALAPPDATA\Groove X"
  keep_data:
SectionEnd
