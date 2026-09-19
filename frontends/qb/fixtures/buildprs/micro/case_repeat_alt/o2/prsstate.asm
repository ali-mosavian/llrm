        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 15:54:34 2026


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
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      07H             , 06H           ; 0:  tkELSE->8
        DB      08H             , 01H           ; 2:  tkX->5
        DB      01H                             ; 4:  Reject
        DB      05H             , 0dbH          ; 5:  tkComma->2
        DB      00H                             ; 7:  Accept
        DB      03H                             ; 8:  EMIT(opStCaseElse)
        DW      opStCaseElse                    
        DB      00H                             ; 11:  Accept

        DB      08H             , 01H           ; 12:  tkX->15
        DB      01H                             ; 14:  Reject
        DB      03H                             ; 15:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 18:  Accept

; state table = 19 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
