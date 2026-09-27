        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 17:32:34 2026


        .xlist
include version.inc
PRSSTATE_ASM = ON
        IncludeOnce opcodes
        IncludeOnce parser
        IncludeOnce pcode
        IncludeOnce qbimsgs
.list

assumes DS,DATA
assumes SS,DATA
assumes ES,NOTHING

sBegin  CP
assumes CS,CP
        EXTRN   NtExp:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD
        DW      28      ; evSwitch
        DW      47      ; fn1arg


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtExp

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      MSG_ExpExp

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      06H             , 01H           ; 0:  fn1arg->3
        DB      01H                             ; 2:  Reject
        DB      03H                             ; 3:  EMIT(opEvCom)
        DW      opEvCom                         
        DB      05H             , 0FFH          ; 6:  evSwitch->Accept
        DB      01H                             ; 8:  Reject

        DB      03H                             ; 9:  EMIT(opEvPen)
        DW      opEvPen                         
        DB      05H             , 0FFH          ; 12:  evSwitch->Accept
        DB      01H                             ; 14:  Reject

        DB      03H                             ; 15:  EMIT(opEvPlay0)
        DW      opEvPlay0                       
        DB      05H             , 0FFH          ; 18:  evSwitch->Accept
        DB      01H                             ; 20:  Reject

        DB      07H             , 01H           ; 21:  Exp->24
        DB      01H                             ; 23:  Reject
        DB      03H                             ; 24:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 27:  Accept

        DB      0dH             , 0dH           ; 28:  tkON->43
        DB      0cH             , 07H           ; 30:  tkOFF->39
        DB      010H            , 01H           ; 32:  tkSTOP->35
        DB      01H                             ; 34:  Reject
        DB      03H                             ; 35:  EMIT(opEvStop)
        DW      opEvStop                        
        DB      00H                             ; 38:  Accept
        DB      03H                             ; 39:  EMIT(opEvOff)
        DW      opEvOff                         
        DB      00H                             ; 42:  Accept
        DB      03H                             ; 43:  EMIT(opEvOn)
        DW      opEvOn                          
        DB      00H                             ; 46:  Accept

        DB      08H             , 01H           ; 47:  tkLParen->50
        DB      01H                             ; 49:  Reject
        DB      07H             , 01H           ; 50:  Exp->53
        DB      01H                             ; 52:  Reject
        DB      09H             , 0FFH          ; 53:  tkRParen->Accept
        DB      01H                             ; 55:  Reject

; state table = 56 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
