; 설치 식별자·파일명은 ASCII(Ddalkkak-Akbo)로 유지하고, 바로가기 라벨만 한글로 만든다.
; → 자동업데이트 정체성/파일명은 영문 그대로(안전), 사용자에게 보이는 아이콘 이름만 "딸깍악보".

; Tauri 기본 "바탕화면 바로가기" 체크박스를 기본 해제(OFF) → 우리가 만든 한글 것과 영문 중복 방지
; (이 파일은 installer.nsi 상단에서 !include 되어 MUI_PAGE_FINISH 보다 먼저 처리됨)
!define MUI_FINISHPAGE_SHOWREADME_NOTCHECKED

!macro NSIS_HOOK_POSTINSTALL
  ; 현재 사용자 기준으로 바로가기 생성 (바탕화면이 OneDrive로 리다이렉트돼도 사용자 데스크톱에 생성)
  SetShellVarContext current
  ; 시작메뉴 바로가기: Tauri 영문 것을 지우고 한글 "딸깍악보"로
  Delete "$SMPROGRAMS\Ddalkkak-Akbo.lnk"
  CreateShortcut "$SMPROGRAMS\딸깍악보.lnk" "$INSTDIR\ddalkkak-akbo.exe"
  ; 바탕화면 바로가기: 한글 "딸깍악보"로 생성 (영문 있으면 지우고)
  Delete "$DESKTOP\Ddalkkak-Akbo.lnk"
  CreateShortcut "$DESKTOP\딸깍악보.lnk" "$INSTDIR\ddalkkak-akbo.exe"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$SMPROGRAMS\딸깍악보.lnk"
  Delete "$DESKTOP\딸깍악보.lnk"
!macroend
