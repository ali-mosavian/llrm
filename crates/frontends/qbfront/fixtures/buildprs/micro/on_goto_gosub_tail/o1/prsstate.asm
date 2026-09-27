        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 17:05:26 2026


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
        EXTRN   NtLabLn:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtExp
        DW      NtLabLn

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      MSG_ExpExp
        DW      MSG_ExpLab

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      05H             , 01H           ; 0:  Exp->3
        DB      01H                             ; 2:  Reject
        DB      0aH             , 07H           ; 3:  tkGOTO->12
        DB      09H             , 01H           ; 5:  tkGOSUB->8
        DB      01H                             ; 7:  Reject
        DB      02H             , 02H           ; 8:  MARK(2)
        DB      04H             , 02H           ; 10:  empty->14
        DB      02H             , 01H           ; 12:  MARK(1)
        DB      06H             , 01H           ; 14:  LabLn->17
        DB      01H                             ; 16:  Reject
        DB      07H             , 01H           ; 17:  tkComma->20
        DB      00H                             ; 19:  Accept
        DB      06H             , 0dbH          ; 20:  LabLn->17
        DB      01H                             ; 22:  Reject

        DB      05H             , 01H           ; 23:  Exp->26
        DB      01H                             ; 25:  Reject
        DB      03H                             ; 26:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 29:  Accept

; state table = 30 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
