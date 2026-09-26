        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 21:38:18 2026


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
        DW      108     ; commaExp
        DW      114     ; exp12
        DW      120     ; expCommaExp
        DW      129     ; fn1arg
        DW      138     ; fn12arg
        DW      150     ; fnBoundArg
        DW      162     ; optCommaExp
        DW      165     ; optFilenum


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
        DB      0dH             , 0FFH          ; 13:  Exp->Accept
        DB      01H                             ; 15:  Reject

        DB      022H            , 01H           ; 16:  tkSEG->19
        DB      01H                             ; 18:  Reject
        DB      017H            , 01H           ; 19:  tkEQ->22
        DB      00H                             ; 21:  Accept
        DB      0dH             , 0FFH          ; 22:  Exp->Accept
        DB      01H                             ; 24:  Reject

        DB      01dH            , 06H           ; 25:  tkFUNCTION->33
        DB      023H            , 01H           ; 27:  tkSUB->30
        DB      01H                             ; 29:  Reject
        DB      011H            , 04H           ; 30:  IdSubDecl->36
        DB      01H                             ; 32:  Reject
        DB      010H            , 01H           ; 33:  IdFuncDecl->36
        DB      01H                             ; 35:  Reject
        DB      02H             , 03H           ; 36:  MARK(3)
        DB      012H            , 0FFH          ; 38:  parms->Accept
        DB      01H                             ; 40:  Reject

        DB      0dH             , 01H           ; 41:  Exp->44
        DB      01H                             ; 43:  Reject
        DB      0bH             , 0FFH          ; 44:  optCommaExp->Accept
        DB      01H                             ; 46:  Reject

        DB      07H             , 01H           ; 47:  expCommaExp->50
        DB      01H                             ; 49:  Reject
        DB      05H             , 01H           ; 50:  commaExp->53
        DB      01H                             ; 52:  Reject
        DB      03H                             ; 53:  EMIT(opStBsave)
        DW      opStBsave                       
        DB      00H                             ; 56:  Accept

        DB      0dH             , 01H           ; 57:  Exp->60
        DB      01H                             ; 59:  Reject
        DB      06H             , 0FFH          ; 60:  exp12->Accept
        DB      01H                             ; 62:  Reject

        DB      08H             , 01H           ; 63:  fn1arg->66
        DB      01H                             ; 65:  Reject
        DB      03H                             ; 66:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 69:  Accept

        DB      09H             , 0FFH          ; 70:  fn12arg->Accept
        DB      01H                             ; 72:  Reject

        DB      0aH             , 0FFH          ; 73:  fnBoundArg->Accept
        DB      01H                             ; 75:  Reject

        DB      0aH             , 0FFH          ; 76:  fnBoundArg->Accept
        DB      01H                             ; 78:  Reject

        DB      014H            , 01H           ; 79:  tkLParen->82
        DB      01H                             ; 81:  Reject
        DB      0dH             , 01H           ; 82:  Exp->85
        DB      01H                             ; 84:  Reject
        DB      016H            , 02H           ; 85:  tkComma->89
        DB      04H             , 03H           ; 87:  empty->92
        DB      0cH             , 01H           ; 89:  optFilenum->92
        DB      01H                             ; 91:  Reject
        DB      015H            , 0FFH          ; 92:  tkRParen->Accept
        DB      01H                             ; 94:  Reject

        DB      014H            , 01H           ; 95:  tkLParen->98
        DB      01H                             ; 97:  Reject
        DB      0cH             , 01H           ; 98:  optFilenum->101
        DB      01H                             ; 100:  Reject
        DB      015H            , 01H           ; 101:  tkRParen->104
        DB      01H                             ; 103:  Reject
        DB      03H                             ; 104:  EMIT(opFnIoctl_)
        DW      opFnIoctl_                      
        DB      00H                             ; 107:  Accept

        DB      016H            , 01H           ; 108:  tkComma->111
        DB      01H                             ; 110:  Reject
        DB      0dH             , 0FFH          ; 111:  Exp->Accept
        DB      01H                             ; 113:  Reject

        DB      05H             , 01H           ; 114:  commaExp->117
        DB      01H                             ; 116:  Reject
        DB      0bH             , 0FFH          ; 117:  optCommaExp->Accept
        DB      01H                             ; 119:  Reject

        DB      0dH             , 01H           ; 120:  Exp->123
        DB      01H                             ; 122:  Reject
        DB      016H            , 01H           ; 123:  tkComma->126
        DB      01H                             ; 125:  Reject
        DB      0dH             , 0FFH          ; 126:  Exp->Accept
        DB      01H                             ; 128:  Reject

        DB      014H            , 01H           ; 129:  tkLParen->132
        DB      01H                             ; 131:  Reject
        DB      0dH             , 01H           ; 132:  Exp->135
        DB      01H                             ; 134:  Reject
        DB      015H            , 0FFH          ; 135:  tkRParen->Accept
        DB      01H                             ; 137:  Reject

        DB      014H            , 01H           ; 138:  tkLParen->141
        DB      01H                             ; 140:  Reject
        DB      0dH             , 01H           ; 141:  Exp->144
        DB      01H                             ; 143:  Reject
        DB      0bH             , 01H           ; 144:  optCommaExp->147
        DB      01H                             ; 146:  Reject
        DB      015H            , 0FFH          ; 147:  tkRParen->Accept
        DB      01H                             ; 149:  Reject

        DB      014H            , 01H           ; 150:  tkLParen->153
        DB      01H                             ; 152:  Reject
        DB      0eH             , 01H           ; 153:  IdArray->156
        DB      01H                             ; 155:  Reject
        DB      0bH             , 01H           ; 156:  optCommaExp->159
        DB      01H                             ; 158:  Reject
        DB      015H            , 0FFH          ; 159:  tkRParen->Accept
        DB      01H                             ; 161:  Reject

        DB      05H             , 0FFH          ; 162:  commaExp->Accept
        DB      00H                             ; 164:  Accept

        DB      013H            , 03H           ; 165:  tkLbs->170
        DB      0dH             , 0FFH          ; 167:  Exp->Accept
        DB      01H                             ; 169:  Reject
        DB      0dH             , 01H           ; 170:  Exp->173
        DB      01H                             ; 172:  Reject
        DB      03H                             ; 173:  EMIT(opLbs)
        DW      opLbs                           
        DB      00H                             ; 176:  Accept

; state table = 177 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
