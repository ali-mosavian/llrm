        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 16:14:21 2026


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
        EXTRN   NtIdFor:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD
        DW      41      ; EMITFFFF


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtExp
        DW      NtIdFor

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      MSG_ExpExp
        DW      MSG_ExpVar

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      07H             , 01H           ; 0:  IdFor->3
        DB      01H                             ; 2:  Reject
        DB      08H             , 01H           ; 3:  tkEQ->6
        DB      01H                             ; 5:  Reject
        DB      06H             , 01H           ; 6:  Exp->9
        DB      01H                             ; 8:  Reject
        DB      0cH             , 01H           ; 9:  tkTO->12
        DB      01H                             ; 11:  Reject
        DB      06H             , 01H           ; 12:  Exp->15
        DB      01H                             ; 14:  Reject
        DB      0bH             , 05H           ; 15:  tkSTEP->22
        DB      03H                             ; 17:  EMIT(opStFor)
        DW      opStFor                         
        DB      04H             , 06H           ; 20:  empty->28
        DB      06H             , 01H           ; 22:  Exp->25
        DB      01H                             ; 24:  Reject
        DB      03H                             ; 25:  EMIT(opStForStep)
        DW      opStForStep                     
        DB      05H             , 01H           ; 28:  EMITFFFF->31
        DB      01H                             ; 30:  Reject
        DB      05H             , 0FFH          ; 31:  EMITFFFF->Accept
        DB      01H                             ; 33:  Reject

        DB      06H             , 01H           ; 34:  Exp->37
        DB      01H                             ; 36:  Reject
        DB      03H                             ; 37:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 40:  Accept

        DB      03H                             ; 41:  EMIT(UNDEFINED)
        DW      UNDEFINED                       
        DB      00H                             ; 44:  Accept

; state table = 45 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
