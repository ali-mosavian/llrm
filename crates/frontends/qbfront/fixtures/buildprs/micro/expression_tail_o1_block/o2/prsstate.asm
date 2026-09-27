        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 18:43:09 2026


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
        EXTRN   NtEndPrint:NEAR
        EXTRN   NtEndPrintExp:NEAR
        EXTRN   NtExp:NEAR
        EXTRN   NtIdArray:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD
        DW      67      ; commaExp
        DW      58      ; exp12
        DW      64      ; expCommaExp
        DW      70      ; fn1arg
        DW      76      ; fn12arg
        DW      82      ; fnBoundArg
        DW      94      ; lbsExpComma
        DW      109     ; optCommaExp
        DW      112     ; optFilenum


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtEndPrint
        DW      NtEndPrintExp
        DW      NtExp
        DW      NtIdArray

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      0       ; EndPrint
        DW      0       ; EndPrintExp
        DW      MSG_ExpExp
        DW      MSG_ExpVar

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      010H            , 03bH          ; 0:  Exp->61
        DB      01H                             ; 2:  Reject

        DB      07H             , 01H           ; 3:  expCommaExp->6
        DB      01H                             ; 5:  Reject
        DB      05H             , 01H           ; 6:  commaExp->9
        DB      01H                             ; 8:  Reject
        DB      03H                             ; 9:  EMIT(opStBsave)
        DW      opStBsave                       
        DB      00H                             ; 12:  Accept

        DB      010H            , 01H           ; 13:  Exp->16
        DB      01H                             ; 15:  Reject
        DB      06H             , 0FFH          ; 16:  exp12->Accept
        DB      01H                             ; 18:  Reject

        DB      08H             , 01H           ; 19:  fn1arg->22
        DB      01H                             ; 21:  Reject
        DB      03H                             ; 22:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 25:  Accept

        DB      09H             , 0FFH          ; 26:  fn12arg->Accept
        DB      01H                             ; 28:  Reject

        DB      0aH             , 0FFH          ; 29:  fnBoundArg->Accept
        DB      01H                             ; 31:  Reject

        DB      013H            , 01H           ; 32:  tkLParen->35
        DB      01H                             ; 34:  Reject
        DB      010H            , 01H           ; 35:  Exp->38
        DB      01H                             ; 37:  Reject
        DB      015H            , 02H           ; 38:  tkComma->42
        DB      04H             , 031H          ; 40:  empty->91
        DB      0dH             , 02fH          ; 42:  optFilenum->91
        DB      01H                             ; 44:  Reject

        DB      013H            , 01H           ; 45:  tkLParen->48
        DB      01H                             ; 47:  Reject
        DB      0dH             , 01H           ; 48:  optFilenum->51
        DB      01H                             ; 50:  Reject
        DB      014H            , 01H           ; 51:  tkRParen->54
        DB      01H                             ; 53:  Reject
        DB      03H                             ; 54:  EMIT(opFnIoctl_)
        DW      opFnIoctl_                      
        DB      00H                             ; 57:  Accept

        DB      05H             , 01H           ; 58:  commaExp->61
        DB      01H                             ; 60:  Reject
        DB      0cH             , 0FFH          ; 61:  optCommaExp->Accept
        DB      01H                             ; 63:  Reject

        DB      010H            , 01H           ; 64:  Exp->67
        DB      01H                             ; 66:  Reject

        DB      015H            , 02dH          ; 67:  tkComma->114
        DB      01H                             ; 69:  Reject

        DB      013H            , 01H           ; 70:  tkLParen->73
        DB      01H                             ; 72:  Reject
        DB      010H            , 010H          ; 73:  Exp->91
        DB      01H                             ; 75:  Reject

        DB      013H            , 01H           ; 76:  tkLParen->79
        DB      01H                             ; 78:  Reject
        DB      010H            , 07H           ; 79:  Exp->88
        DB      01H                             ; 81:  Reject

        DB      013H            , 01H           ; 82:  tkLParen->85
        DB      01H                             ; 84:  Reject
        DB      011H            , 01H           ; 85:  IdArray->88
        DB      01H                             ; 87:  Reject
        DB      0cH             , 01H           ; 88:  optCommaExp->91
        DB      01H                             ; 90:  Reject
        DB      014H            , 0FFH          ; 91:  tkRParen->Accept
        DB      01H                             ; 93:  Reject

        DB      012H            , 01H           ; 94:  tkLbs->97
        DB      01H                             ; 96:  Reject
        DB      010H            , 01H           ; 97:  Exp->100
        DB      01H                             ; 99:  Reject
        DB      03H                             ; 100:  EMIT(opLbs)
        DW      opLbs                           
        DB      03H                             ; 103:  EMIT(opChanOut)
        DW      opChanOut                       
        DB      015H            , 0FFH          ; 106:  tkComma->Accept
        DB      01H                             ; 108:  Reject

        DB      05H             , 0FFH          ; 109:  commaExp->Accept
        DB      00H                             ; 111:  Accept

        DB      012H            , 03H           ; 112:  tkLbs->117
        DB      010H            , 0FFH          ; 114:  Exp->Accept
        DB      01H                             ; 116:  Reject
        DB      010H            , 01H           ; 117:  Exp->120
        DB      01H                             ; 119:  Reject
        DB      03H                             ; 120:  EMIT(opLbs)
        DW      opLbs                           
        DB      00H                             ; 123:  Accept

; state table = 124 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
