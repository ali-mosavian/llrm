        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 17:32:35 2026


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
        DW      26      ; evSwitch
        DW      45      ; fn1arg


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
        DB      04H             , 08H           ; 6:  empty->16

        DB      03H                             ; 8:  EMIT(opEvPen)
        DW      opEvPen                         
        DB      04H             , 03H           ; 11:  empty->16

        DB      03H                             ; 13:  EMIT(opEvPlay0)
        DW      opEvPlay0                       
        DB      05H             , 0FFH          ; 16:  evSwitch->Accept
        DB      01H                             ; 18:  Reject

        DB      07H             , 01H           ; 19:  Exp->22
        DB      01H                             ; 21:  Reject
        DB      03H                             ; 22:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 25:  Accept

        DB      0dH             , 0dH           ; 26:  tkON->41
        DB      0cH             , 07H           ; 28:  tkOFF->37
        DB      010H            , 01H           ; 30:  tkSTOP->33
        DB      01H                             ; 32:  Reject
        DB      03H                             ; 33:  EMIT(opEvStop)
        DW      opEvStop                        
        DB      00H                             ; 36:  Accept
        DB      03H                             ; 37:  EMIT(opEvOff)
        DW      opEvOff                         
        DB      00H                             ; 40:  Accept
        DB      03H                             ; 41:  EMIT(opEvOn)
        DW      opEvOn                          
        DB      00H                             ; 44:  Accept

        DB      08H             , 01H           ; 45:  tkLParen->48
        DB      01H                             ; 47:  Reject
        DB      07H             , 01H           ; 48:  Exp->51
        DB      01H                             ; 50:  Reject
        DB      09H             , 0FFH          ; 51:  tkRParen->Accept
        DB      01H                             ; 53:  Reject

; state table = 54 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
