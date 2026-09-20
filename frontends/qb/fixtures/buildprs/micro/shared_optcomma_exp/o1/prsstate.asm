        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 15:42:15 2026


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
        DW      16      ; commaExp
        DW      22      ; exp12
        DW      28      ; optCommaExp


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtExp

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      MSG_ExpExp

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      08H             , 017H          ; 0:  Exp->25
        DB      01H                             ; 2:  Reject

        DB      08H             , 01H           ; 3:  Exp->6
        DB      01H                             ; 5:  Reject
        DB      06H             , 0FFH          ; 6:  exp12->Accept
        DB      01H                             ; 8:  Reject

        DB      08H             , 01H           ; 9:  Exp->12
        DB      01H                             ; 11:  Reject
        DB      03H                             ; 12:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 15:  Accept

        DB      09H             , 01H           ; 16:  tkComma->19
        DB      01H                             ; 18:  Reject
        DB      08H             , 0FFH          ; 19:  Exp->Accept
        DB      01H                             ; 21:  Reject

        DB      05H             , 01H           ; 22:  commaExp->25
        DB      01H                             ; 24:  Reject
        DB      07H             , 0FFH          ; 25:  optCommaExp->Accept
        DB      01H                             ; 27:  Reject

        DB      05H             , 0FFH          ; 28:  commaExp->Accept
        DB      00H                             ; 30:  Accept

; state table = 31 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
