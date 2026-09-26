        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 16:12:58 2026


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
        EXTRN   NtIdAryElem:NEAR
        EXTRN   NtoptFilenum:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD
        DW      39      ; commaExp


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtExp
        DW      NtIdAryElem
        DW      NtoptFilenum

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      MSG_ExpExp
        DW      MSG_ExpVar
        DW      0       ; optFilenum

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      08H             , 01H           ; 0:  optFilenum->3
        DB      01H                             ; 2:  Reject
        DB      03H                             ; 3:  EMIT(opFieldInit)
        DW      opFieldInit                     
        DB      05H             , 01H           ; 6:  commaExp->9
        DB      01H                             ; 8:  Reject
        DB      0bH             , 01H           ; 9:  tkAS->12
        DB      01H                             ; 11:  Reject
        DB      07H             , 01H           ; 12:  IdAryElem->15
        DB      01H                             ; 14:  Reject
        DB      03H                             ; 15:  EMIT(opFieldItem)
        DW      opFieldItem                     
        DB      05H             , 01H           ; 18:  commaExp->21
        DB      00H                             ; 20:  Accept
        DB      0bH             , 01H           ; 21:  tkAS->24
        DB      01H                             ; 23:  Reject
        DB      07H             , 01H           ; 24:  IdAryElem->27
        DB      01H                             ; 26:  Reject
        DB      03H                             ; 27:  EMIT(opFieldItem)
        DW      opFieldItem                     
        DB      04H             , 0d2H          ; 30:  empty->18

        DB      07H             , 01H           ; 32:  IdAryElem->35
        DB      01H                             ; 34:  Reject
        DB      03H                             ; 35:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 38:  Accept

        DB      09H             , 01H           ; 39:  tkComma->42
        DB      01H                             ; 41:  Reject
        DB      06H             , 0FFH          ; 42:  Exp->Accept
        DB      01H                             ; 44:  Reject

; state table = 45 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
