        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 17:04:41 2026


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
        EXTRN   NtoptFilenum:NEAR
        EXTRN   NtExp:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtoptFilenum
        DW      NtExp

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      0       ; optFilenum
        DW      MSG_ExpExp

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      05H             , 01H           ; 0:  optFilenum->3
        DB      01H                             ; 2:  Reject
        DB      07H             , 01H           ; 3:  tkComma->6
        DB      00H                             ; 5:  Accept
        DB      06H             , 08H           ; 6:  Exp->16
        DB      0aH             , 01H           ; 8:  tkTO->11
        DB      01H                             ; 10:  Reject
        DB      02H             , 03H           ; 11:  MARK(3)
        DB      06H             , 0FFH          ; 13:  Exp->Accept
        DB      01H                             ; 15:  Reject
        DB      02H             , 01H           ; 16:  MARK(1)
        DB      0aH             , 01H           ; 18:  tkTO->21
        DB      00H                             ; 20:  Accept
        DB      02H             , 02H           ; 21:  MARK(2)
        DB      06H             , 0FFH          ; 23:  Exp->Accept
        DB      01H                             ; 25:  Reject

        DB      06H             , 01H           ; 26:  Exp->29
        DB      01H                             ; 28:  Reject
        DB      03H                             ; 29:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 32:  Accept

; state table = 33 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
