        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 21:38:21 2026


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
        EXTRN   NtIdArray:NEAR
        EXTRN   NtIdFn:NEAR
        EXTRN   NtIdFuncDecl:NEAR
        EXTRN   NtIdSubDecl:NEAR
        EXTRN   Ntparms:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD
        DW      106     ; commaExp
        DW      97      ; exp12
        DW      103     ; expCommaExp
        DW      109     ; fn1arg
        DW      115     ; fn12arg
        DW      121     ; fnBoundArg
        DW      133     ; optCommaExp
        DW      136     ; optFilenum


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtExp
        DW      NtIdArray
        DW      NtIdFn
        DW      NtIdFuncDecl
        DW      NtIdSubDecl
        DW      Ntparms

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      MSG_ExpExp
        DW      MSG_ExpVar
        DW      0       ; IdFn
        DW      0       ; IdFuncDecl
        DW      0       ; IdSubDecl
        DW      0       ; parms

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      0fH             , 01H           ; 0:  IdFn->3
        DB      01H                             ; 2:  Reject
        DB      02H             , 03H           ; 3:  MARK(3)
        DB      012H            , 01H           ; 5:  parms->8
        DB      01H                             ; 7:  Reject
        DB      017H            , 01H           ; 8:  tkEQ->11
        DB      00H                             ; 10:  Accept
        DB      02H             , 05H           ; 11:  MARK(5)
        DB      04H             , 0e0H, 08aH    ; 13:  empty->138

        DB      022H            , 01H           ; 16:  tkSEG->19
        DB      01H                             ; 18:  Reject
        DB      017H            , 0e0H, 08aH    ; 19:  tkEQ->138
        DB      00H                             ; 22:  Accept

        DB      01dH            , 06H           ; 23:  tkFUNCTION->31
        DB      023H            , 01H           ; 25:  tkSUB->28
        DB      01H                             ; 27:  Reject
        DB      011H            , 04H           ; 28:  IdSubDecl->34
        DB      01H                             ; 30:  Reject
        DB      010H            , 01H           ; 31:  IdFuncDecl->34
        DB      01H                             ; 33:  Reject
        DB      02H             , 03H           ; 34:  MARK(3)
        DB      012H            , 0FFH          ; 36:  parms->Accept
        DB      01H                             ; 38:  Reject

        DB      0dH             , 03bH          ; 39:  Exp->100
        DB      01H                             ; 41:  Reject

        DB      07H             , 01H           ; 42:  expCommaExp->45
        DB      01H                             ; 44:  Reject
        DB      05H             , 01H           ; 45:  commaExp->48
        DB      01H                             ; 47:  Reject
        DB      03H                             ; 48:  EMIT(opStBsave)
        DW      opStBsave                       
        DB      00H                             ; 51:  Accept

        DB      0dH             , 01H           ; 52:  Exp->55
        DB      01H                             ; 54:  Reject
        DB      06H             , 0FFH          ; 55:  exp12->Accept
        DB      01H                             ; 57:  Reject

        DB      08H             , 01H           ; 58:  fn1arg->61
        DB      01H                             ; 60:  Reject
        DB      03H                             ; 61:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 64:  Accept

        DB      09H             , 0FFH          ; 65:  fn12arg->Accept
        DB      01H                             ; 67:  Reject

        DB      0aH             , 0FFH          ; 68:  fnBoundArg->Accept
        DB      01H                             ; 70:  Reject

        DB      014H            , 01H           ; 71:  tkLParen->74
        DB      01H                             ; 73:  Reject
        DB      0dH             , 01H           ; 74:  Exp->77
        DB      01H                             ; 76:  Reject
        DB      016H            , 02H           ; 77:  tkComma->81
        DB      04H             , 031H          ; 79:  empty->130
        DB      0cH             , 02fH          ; 81:  optFilenum->130
        DB      01H                             ; 83:  Reject

        DB      014H            , 01H           ; 84:  tkLParen->87
        DB      01H                             ; 86:  Reject
        DB      0cH             , 01H           ; 87:  optFilenum->90
        DB      01H                             ; 89:  Reject
        DB      015H            , 01H           ; 90:  tkRParen->93
        DB      01H                             ; 92:  Reject
        DB      03H                             ; 93:  EMIT(opFnIoctl_)
        DW      opFnIoctl_                      
        DB      00H                             ; 96:  Accept

        DB      05H             , 01H           ; 97:  commaExp->100
        DB      01H                             ; 99:  Reject
        DB      0bH             , 0FFH          ; 100:  optCommaExp->Accept
        DB      01H                             ; 102:  Reject

        DB      0dH             , 01H           ; 103:  Exp->106
        DB      01H                             ; 105:  Reject

        DB      016H            , 01eH          ; 106:  tkComma->138
        DB      01H                             ; 108:  Reject

        DB      014H            , 01H           ; 109:  tkLParen->112
        DB      01H                             ; 111:  Reject
        DB      0dH             , 010H          ; 112:  Exp->130
        DB      01H                             ; 114:  Reject

        DB      014H            , 01H           ; 115:  tkLParen->118
        DB      01H                             ; 117:  Reject
        DB      0dH             , 07H           ; 118:  Exp->127
        DB      01H                             ; 120:  Reject

        DB      014H            , 01H           ; 121:  tkLParen->124
        DB      01H                             ; 123:  Reject
        DB      0eH             , 01H           ; 124:  IdArray->127
        DB      01H                             ; 126:  Reject
        DB      0bH             , 01H           ; 127:  optCommaExp->130
        DB      01H                             ; 129:  Reject
        DB      015H            , 0FFH          ; 130:  tkRParen->Accept
        DB      01H                             ; 132:  Reject

        DB      05H             , 0FFH          ; 133:  commaExp->Accept
        DB      00H                             ; 135:  Accept

        DB      013H            , 03H           ; 136:  tkLbs->141
        DB      0dH             , 0FFH          ; 138:  Exp->Accept
        DB      01H                             ; 140:  Reject
        DB      0dH             , 01H           ; 141:  Exp->144
        DB      01H                             ; 143:  Reject
        DB      03H                             ; 144:  EMIT(opLbs)
        DW      opLbs                           
        DB      00H                             ; 147:  Accept

; state table = 148 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
