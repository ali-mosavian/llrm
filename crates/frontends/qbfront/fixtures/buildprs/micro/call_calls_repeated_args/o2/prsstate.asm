        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 17:37:43 2026


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
        EXTRN   NtIdCallArg:NEAR
        EXTRN   NtIdSubRef:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtExp
        DW      NtIdCallArg
        DW      NtIdSubRef

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      MSG_ExpExp
        DW      MSG_ExpIdCallArg
        DW      0       ; IdSubRef

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      02H             , 01H           ; 0:  MARK(1)
        DB      07H             , 01H           ; 2:  IdSubRef->5
        DB      01H                             ; 4:  Reject
        DB      08H             , 01H           ; 5:  tkLParen->8
        DB      00H                             ; 7:  Accept
        DB      06H             , 0cH           ; 8:  IdCallArg->22
        DB      01H                             ; 10:  Reject

        DB      02H             , 01H           ; 11:  MARK(1)
        DB      07H             , 01H           ; 13:  IdSubRef->16
        DB      01H                             ; 15:  Reject
        DB      08H             , 01H           ; 16:  tkLParen->19
        DB      00H                             ; 18:  Accept
        DB      05H             , 01H           ; 19:  Exp->22
        DB      01H                             ; 21:  Reject
        DB      0aH             , 0d0H          ; 22:  tkComma->8
        DB      09H             , 0FFH          ; 24:  tkRParen->Accept
        DB      01H                             ; 26:  Reject

        DB      05H             , 01H           ; 27:  Exp->30
        DB      01H                             ; 29:  Reject
        DB      03H                             ; 30:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 33:  Accept

; state table = 34 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
