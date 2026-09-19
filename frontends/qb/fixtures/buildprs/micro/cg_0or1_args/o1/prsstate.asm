        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 14:51:41 2026


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
        DB      06H             , 0FFH          ; 0:  tkX->Accept
        DB      00H                             ; 2:  Accept

; state table = 3 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
