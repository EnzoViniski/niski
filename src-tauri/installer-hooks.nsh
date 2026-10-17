; Prepara a troca dos arquivos: tudo que o OMNI deixa rodando precisa ter saído — e os .exe
; precisam estar liberados — antes de o NSIS copiar a versão nova.
;
; O engine roda destacado (DETACHED_PROCESS) e **sobrevive ao fechamento da janela** — é o que
; permite o acesso pelo celular com o app fechado. O efeito colateral é que, numa atualização, o
; `omni-engine.exe` continua em uso e o NSIS não consegue substituí-lo: na 0.3.1 a instalação
; terminou "com sucesso" deixando o engine da versão anterior no lugar.
;
; E o app em si: na atualização automática o updater abre este instalador e só então o app termina
; de fechar. Matar e seguir em frente não basta — o Windows leva um instante para soltar o arquivo,
; e a cópia parava em "Erro ao abrir o arquivo para escrita", com Repetir. Por isso: espera cada
; processo sair (forçando depois de um tempo) e só segue quando o .exe aceita ser aberto para escrita.
;
; `taskkill` sem `/T`: as PTYs filhas do engine morrem junto quando o ConPTY fecha, que é o esperado
; numa atualização. Processo que não estava rodando é ignorado.

!include LogicLib.nsh

; Espera `exe` sair. Até 2,5 s por conta própria; depois força o fechamento. Desiste em 10 s.
!macro OMNI_ESPERA_SAIR exe
  StrCpy $R9 0
  ${Do}
    !if "${INSTALLMODE}" == "currentUser"
      nsis_tauri_utils::FindProcessCurrentUser "${exe}"
    !else
      nsis_tauri_utils::FindProcess "${exe}"
    !endif
    Pop $R8
    ${If} $R8 <> 0
      ${ExitDo}
    ${EndIf}
    ${If} $R9 >= 10
      nsExec::Exec 'taskkill /F /IM ${exe}'
      Pop $R8
    ${EndIf}
    Sleep 250
    IntOp $R9 $R9 + 1
  ${LoopUntil} $R9 >= 40
!macroend

; Espera o arquivo aceitar escrita (o Windows solta o .exe um pouco depois de o processo sair).
!macro OMNI_ESPERA_LIBERAR arquivo
  StrCpy $R9 0
  ${Do}
    ${IfNot} ${FileExists} "${arquivo}"
      ${ExitDo}
    ${EndIf}
    ClearErrors
    FileOpen $R8 "${arquivo}" a
    ${IfNot} ${Errors}
      FileClose $R8
      ${ExitDo}
    ${EndIf}
    Sleep 250
    IntOp $R9 $R9 + 1
  ${LoopUntil} $R9 >= 40
!macroend

!macro OMNI_LIBERA_TUDO
  ; O engine não fecha sozinho: vai direto para o taskkill.
  nsExec::Exec 'taskkill /F /IM omni-engine.exe'
  Pop $0
  !insertmacro OMNI_ESPERA_SAIR "${MAINBINARYNAME}.exe"
  !insertmacro OMNI_ESPERA_SAIR "omni-engine.exe"
  !insertmacro OMNI_ESPERA_SAIR "omni-shim.exe"
  !insertmacro OMNI_ESPERA_LIBERAR "$INSTDIR\${MAINBINARYNAME}.exe"
  !insertmacro OMNI_ESPERA_LIBERAR "$INSTDIR\omni-engine.exe"
  !insertmacro OMNI_ESPERA_LIBERAR "$INSTDIR\omni-shim.exe"
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro OMNI_LIBERA_TUDO
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro OMNI_LIBERA_TUDO
!macroend
