# Bản cài Windows của Groove X (groove-gd1_27). Dựng bằng `packaging/windows/build-installer.ps1`, không gọi tay: script truyền
# VERSION, VERSION4, STAGE, ICON, OUTFILE qua /D và đọc tệp này với /INPUTCHARSET UTF8.
#
# Cài theo người dùng, không cần admin, không ký (ký là dịch vụ trả phí; SmartScreen sẽ cảnh báo). Liên kết định dạng theo đặc tả
# 5.6: chỉ ghi ProgID và `OpenWithProgids`, tức là hiện trong "Mở bằng"; Windows không cho tự đặt mặc định, người dùng tự chọn.
# Danh sách đuôi phải khớp `AUDIO_EXTENSIONS`: `crates/groove-app/tests/packaging.rs` soát.
#
# Phiên đang chạy: không giết theo tên, máy dev có thể đang chạy bản khác. Exe trong thư mục cài còn bị giữ thì hỏi đóng rồi thử lại;
# cài im lặng thì dừng, không ghi đè nửa chừng. Phiên sau chỉ chuyển tệp cho phiên chính cùng thư mục (`instance::sys::prepare_server`).
# Gỡ cài không đụng dữ liệu của người dùng (`%APPDATA%\Groove X`), và chỉ xoá thứ bộ cài đã đặt.

Unicode true
SetCompressor /SOLID lzma
ManifestDPIAware true
RequestExecutionLevel user

!include "MUI2.nsh"

!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\GrooveX"
!define UNINST_EXE "uninstall.exe"
!define PROG_ID "GrooveX.AudioFile"
!define CLASSES "Software\Classes"
# 250 ms mỗi lượt: chờ tối đa 5 s.
!define LOCK_ROUNDS 20

Name "Groove X"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\Groove X"
InstallDirRegKey HKCU "${UNINST_KEY}" "InstallLocation"
BrandingText "Groove X ${VERSION}"

VIProductVersion "${VERSION4}"
VIAddVersionKey /LANG=0 "ProductName" "Groove X"
VIAddVersionKey /LANG=0 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=0 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=0 "FileDescription" "Groove X setup"
VIAddVersionKey /LANG=0 "LegalCopyright" "Groove X"

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\groove.exe"
!define MUI_FINISHPAGE_RUN_TEXT "$(run_groove)"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

# Ngôn ngữ đầu là mặc định khi máy không dùng ngôn ngữ nào trong danh sách.
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Vietnamese"

LangString run_groove ${LANG_ENGLISH} "Run Groove X"
LangString run_groove ${LANG_VIETNAMESE} "Chạy Groove X"
LangString still_running ${LANG_ENGLISH} "Groove X is still running. Close it, then choose Retry."
LangString still_running ${LANG_VIETNAMESE} "Groove X vẫn đang chạy. Hãy đóng nó rồi chọn Retry."
LangString file_type ${LANG_ENGLISH} "Groove X audio file"
LangString file_type ${LANG_VIETNAMESE} "Tệp nhạc Groove X"

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

!macro AssociateExtension EXT
  WriteRegStr HKCU "${CLASSES}\.${EXT}\OpenWithProgids" "${PROG_ID}" ""
  WriteRegStr HKCU "${CLASSES}\Applications\groove.exe\SupportedTypes" ".${EXT}" ""
!macroend

!macro ForgetExtension EXT
  DeleteRegValue HKCU "${CLASSES}\.${EXT}\OpenWithProgids" "${PROG_ID}"
  # Khoá rỗng mới xoá: ứng dụng khác có thể đang đứng cùng chỗ.
  DeleteRegKey /ifempty HKCU "${CLASSES}\.${EXT}\OpenWithProgids"
  DeleteRegKey /ifempty HKCU "${CLASSES}\.${EXT}"
!macroend

# SHCNE_ASSOCCHANGED: Explorer đọc lại liên kết, không phải khởi động lại.
!macro NotifyShell
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

Section "Groove X" SecMain
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
  CreateShortcut "$SMPROGRAMS\Groove X.lnk" "$INSTDIR\groove.exe"

  WriteRegStr HKCU "${CLASSES}\${PROG_ID}" "" "$(file_type)"
  WriteRegStr HKCU "${CLASSES}\${PROG_ID}\DefaultIcon" "" "$INSTDIR\groove.exe,0"
  WriteRegStr HKCU "${CLASSES}\${PROG_ID}\shell\open\command" "" '"$INSTDIR\groove.exe" "%1"'
  WriteRegStr HKCU "${CLASSES}\Applications\groove.exe" "FriendlyAppName" "Groove X"
  WriteRegStr HKCU "${CLASSES}\Applications\groove.exe\shell\open\command" "" '"$INSTDIR\groove.exe" "%1"'
  !insertmacro AssociateExtension "mp3"
  !insertmacro AssociateExtension "flac"
  !insertmacro AssociateExtension "wav"
  !insertmacro AssociateExtension "m4a"
  !insertmacro AssociateExtension "m4r"
  !insertmacro AssociateExtension "ogg"
  !insertmacro NotifyShell

  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "Groove X"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "Groove X"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\groove.exe,0"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\${UNINST_EXE}"'
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\${UNINST_EXE}" /S'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  SectionGetSize ${SecMain} $0
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" $0
SectionEnd

Section "Uninstall"
  Call un.StopApp

  !insertmacro ForgetExtension "mp3"
  !insertmacro ForgetExtension "flac"
  !insertmacro ForgetExtension "wav"
  !insertmacro ForgetExtension "m4a"
  !insertmacro ForgetExtension "m4r"
  !insertmacro ForgetExtension "ogg"
  DeleteRegKey HKCU "${CLASSES}\${PROG_ID}"
  DeleteRegKey HKCU "${CLASSES}\Applications\groove.exe"
  !insertmacro NotifyShell

  Delete "$SMPROGRAMS\Groove X.lnk"
  DeleteRegKey HKCU "${UNINST_KEY}"

  Delete "$INSTDIR\groove.exe"
  RMDir /r "$INSTDIR\licenses"
  Delete "$INSTDIR\${UNINST_EXE}"
  # Không /r: thư mục còn tệp lạ thì để nguyên.
  RMDir "$INSTDIR"
SectionEnd
