        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 17:22:26 2026


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
        EXTRN   NtLit0:NEAR
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
        DW      NtLit0
        DW      NtLabLn

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      MSG_ExpExp
        DW      MSG_ExpLit0
        DW      MSG_ExpLab

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      02H             , 01H           ; 0:  MARK(1)
        DB      06H             , 0bH           ; 2:  Lit0->15
        DB      07H             , 06H           ; 4:  LabLn->12
        DB      09H             , 01H           ; 6:  tkNEXT->9
        DB      00H                             ; 8:  Accept
        DB      02H             , 04H           ; 9:  MARK(4)
        DB      00H                             ; 11:  Accept
        DB      02H             , 02H           ; 12:  MARK(2)
        DB      00H                             ; 14:  Accept
        DB      02H             , 03H           ; 15:  MARK(3)
        DB      00H                             ; 17:  Accept

        DB      05H             , 01H           ; 18:  Exp->21
        DB      01H                             ; 20:  Reject
        DB      03H                             ; 21:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 24:  Accept

; state table = 25 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
