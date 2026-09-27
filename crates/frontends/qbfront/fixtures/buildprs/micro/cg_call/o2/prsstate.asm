        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 15:06:08 2026


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
        DB      02H             , 01H           ; 0:  MARK(1)
        DB      09H             , 01H           ; 2:  tkX->5
        DB      01H                             ; 4:  Reject
        DB      05H             , 01H           ; 5:  tkLParen->8
        DB      00H                             ; 7:  Accept
        DB      09H             , 01H           ; 8:  tkX->11
        DB      01H                             ; 10:  Reject
        DB      07H             , 0dbH          ; 11:  tkComma->8
        DB      06H             , 0FFH          ; 13:  tkRParen->Accept
        DB      01H                             ; 15:  Reject

; state table = 16 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
