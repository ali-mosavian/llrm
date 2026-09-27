        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 15:42:13 2026


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
        DW      19      ; commaExp
        DW      25      ; exp12
        DW      31      ; optCommaExp


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtExp

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      MSG_ExpExp

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      08H             , 01H           ; 0:  Exp->3
        DB      01H                             ; 2:  Reject
        DB      07H             , 0FFH          ; 3:  optCommaExp->Accept
        DB      01H                             ; 5:  Reject

        DB      08H             , 01H           ; 6:  Exp->9
        DB      01H                             ; 8:  Reject
        DB      06H             , 0FFH          ; 9:  exp12->Accept
        DB      01H                             ; 11:  Reject

        DB      08H             , 01H           ; 12:  Exp->15
        DB      01H                             ; 14:  Reject
        DB      03H                             ; 15:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 18:  Accept

        DB      09H             , 01H           ; 19:  tkComma->22
        DB      01H                             ; 21:  Reject
        DB      08H             , 0FFH          ; 22:  Exp->Accept
        DB      01H                             ; 24:  Reject

        DB      05H             , 01H           ; 25:  commaExp->28
        DB      01H                             ; 27:  Reject
        DB      07H             , 0FFH          ; 28:  optCommaExp->Accept
        DB      01H                             ; 30:  Reject

        DB      05H             , 0FFH          ; 31:  commaExp->Accept
        DB      00H                             ; 33:  Accept

; state table = 34 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
