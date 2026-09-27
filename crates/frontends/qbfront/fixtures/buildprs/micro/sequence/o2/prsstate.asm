        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 15:06:02 2026


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
        DB      06H             , 01H           ; 0:  tkB->3
        DB      01H                             ; 2:  Reject
        DB      03H                             ; 3:  EMIT(opStBeep)
        DW      opStBeep                        
        DB      00H                             ; 6:  Accept

; state table = 7 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
