        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 15:56:32 2026


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
        EXTRN   NtcoordStep:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD
        DW      29      ; commaExp


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtExp
        DW      NtcoordStep

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      MSG_ExpExp
        DW      MSG_ExpExp

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      07H             , 01H           ; 0:  coordStep->3
        DB      01H                             ; 2:  Reject
        DB      05H             , 01H           ; 3:  commaExp->6
        DB      01H                             ; 5:  Reject
        DB      08H             , 01H           ; 6:  tkComma->9
        DB      00H                             ; 8:  Accept
        DB      06H             , 02H           ; 9:  Exp->13
        DB      04H             , 02H           ; 11:  empty->15
        DB      02H             , 01H           ; 13:  MARK(1)
        DB      05H             , 01H           ; 15:  commaExp->18
        DB      01H                             ; 17:  Reject
        DB      03H                             ; 18:  EMIT(opCircleStart)
        DW      opCircleStart                   
        DB      00H                             ; 21:  Accept

        DB      06H             , 01H           ; 22:  Exp->25
        DB      01H                             ; 24:  Reject
        DB      03H                             ; 25:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 28:  Accept

        DB      08H             , 01H           ; 29:  tkComma->32
        DB      01H                             ; 31:  Reject
        DB      06H             , 0FFH          ; 32:  Exp->Accept
        DB      01H                             ; 34:  Reject

; state table = 35 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
