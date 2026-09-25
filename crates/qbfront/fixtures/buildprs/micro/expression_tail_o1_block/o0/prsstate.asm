        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 18:43:07 2026


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
        DW      73      ; exp12
        DW      79      ; expCommaExp
        DW      88      ; fn1arg
        DW      97      ; fn12arg
        DW      109     ; fnBoundArg
        DW      121     ; lbsExpComma
        DW      136     ; optCommaExp
        DW      139     ; optFilenum


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
        DB      010H            , 01H           ; 0:  Exp->3
        DB      01H                             ; 2:  Reject
        DB      0cH             , 0FFH          ; 3:  optCommaExp->Accept
        DB      01H                             ; 5:  Reject

        DB      07H             , 01H           ; 6:  expCommaExp->9
        DB      01H                             ; 8:  Reject
        DB      05H             , 01H           ; 9:  commaExp->12
        DB      01H                             ; 11:  Reject
        DB      03H                             ; 12:  EMIT(opStBsave)
        DW      opStBsave                       
        DB      00H                             ; 15:  Accept

        DB      010H            , 01H           ; 16:  Exp->19
        DB      01H                             ; 18:  Reject
        DB      06H             , 0FFH          ; 19:  exp12->Accept
        DB      01H                             ; 21:  Reject

        DB      08H             , 01H           ; 22:  fn1arg->25
        DB      01H                             ; 24:  Reject
        DB      03H                             ; 25:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 28:  Accept

        DB      09H             , 0FFH          ; 29:  fn12arg->Accept
        DB      01H                             ; 31:  Reject

        DB      0aH             , 0FFH          ; 32:  fnBoundArg->Accept
        DB      01H                             ; 34:  Reject

        DB      0aH             , 0FFH          ; 35:  fnBoundArg->Accept
        DB      01H                             ; 37:  Reject

        DB      013H            , 01H           ; 38:  tkLParen->41
        DB      01H                             ; 40:  Reject
        DB      010H            , 01H           ; 41:  Exp->44
        DB      01H                             ; 43:  Reject
        DB      015H            , 02H           ; 44:  tkComma->48
        DB      04H             , 03H           ; 46:  empty->51
        DB      0dH             , 01H           ; 48:  optFilenum->51
        DB      01H                             ; 50:  Reject
        DB      014H            , 0FFH          ; 51:  tkRParen->Accept
        DB      01H                             ; 53:  Reject

        DB      013H            , 01H           ; 54:  tkLParen->57
        DB      01H                             ; 56:  Reject
        DB      0dH             , 01H           ; 57:  optFilenum->60
        DB      01H                             ; 59:  Reject
        DB      014H            , 01H           ; 60:  tkRParen->63
        DB      01H                             ; 62:  Reject
        DB      03H                             ; 63:  EMIT(opFnIoctl_)
        DW      opFnIoctl_                      
        DB      00H                             ; 66:  Accept

        DB      015H            , 01H           ; 67:  tkComma->70
        DB      01H                             ; 69:  Reject
        DB      010H            , 0FFH          ; 70:  Exp->Accept
        DB      01H                             ; 72:  Reject

        DB      05H             , 01H           ; 73:  commaExp->76
        DB      01H                             ; 75:  Reject
        DB      0cH             , 0FFH          ; 76:  optCommaExp->Accept
        DB      01H                             ; 78:  Reject

        DB      010H            , 01H           ; 79:  Exp->82
        DB      01H                             ; 81:  Reject
        DB      015H            , 01H           ; 82:  tkComma->85
        DB      01H                             ; 84:  Reject
        DB      010H            , 0FFH          ; 85:  Exp->Accept
        DB      01H                             ; 87:  Reject

        DB      013H            , 01H           ; 88:  tkLParen->91
        DB      01H                             ; 90:  Reject
        DB      010H            , 01H           ; 91:  Exp->94
        DB      01H                             ; 93:  Reject
        DB      014H            , 0FFH          ; 94:  tkRParen->Accept
        DB      01H                             ; 96:  Reject

        DB      013H            , 01H           ; 97:  tkLParen->100
        DB      01H                             ; 99:  Reject
        DB      010H            , 01H           ; 100:  Exp->103
        DB      01H                             ; 102:  Reject
        DB      0cH             , 01H           ; 103:  optCommaExp->106
        DB      01H                             ; 105:  Reject
        DB      014H            , 0FFH          ; 106:  tkRParen->Accept
        DB      01H                             ; 108:  Reject

        DB      013H            , 01H           ; 109:  tkLParen->112
        DB      01H                             ; 111:  Reject
        DB      011H            , 01H           ; 112:  IdArray->115
        DB      01H                             ; 114:  Reject
        DB      0cH             , 01H           ; 115:  optCommaExp->118
        DB      01H                             ; 117:  Reject
        DB      014H            , 0FFH          ; 118:  tkRParen->Accept
        DB      01H                             ; 120:  Reject

        DB      012H            , 01H           ; 121:  tkLbs->124
        DB      01H                             ; 123:  Reject
        DB      010H            , 01H           ; 124:  Exp->127
        DB      01H                             ; 126:  Reject
        DB      03H                             ; 127:  EMIT(opLbs)
        DW      opLbs                           
        DB      03H                             ; 130:  EMIT(opChanOut)
        DW      opChanOut                       
        DB      015H            , 0FFH          ; 133:  tkComma->Accept
        DB      01H                             ; 135:  Reject

        DB      05H             , 0FFH          ; 136:  commaExp->Accept
        DB      00H                             ; 138:  Accept

        DB      012H            , 03H           ; 139:  tkLbs->144
        DB      010H            , 0FFH          ; 141:  Exp->Accept
        DB      01H                             ; 143:  Reject
        DB      010H            , 01H           ; 144:  Exp->147
        DB      01H                             ; 146:  Reject
        DB      03H                             ; 147:  EMIT(opLbs)
        DW      opLbs                           
        DB      00H                             ; 150:  Accept

; state table = 151 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
