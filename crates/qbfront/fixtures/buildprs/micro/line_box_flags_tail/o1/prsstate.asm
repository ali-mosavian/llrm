        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 16:53:07 2026


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
        EXTRN   NtcoordStep:NEAR
        EXTRN   Ntcoord2Step:NEAR
        EXTRN   NtExp:NEAR
        EXTRN   NtRwB:NEAR
        EXTRN   NtRwF:NEAR
        EXTRN   NtRwBF:NEAR
        EXTRN   Ntfn1arg:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD
        DW      51      ; commaExp


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtcoordStep
        DW      Ntcoord2Step
        DW      NtExp
        DW      NtRwB
        DW      NtRwF
        DW      NtRwBF
        DW      Ntfn1arg

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      MSG_ExpExp
        DW      MSG_ExpExp
        DW      MSG_ExpExp
        DW      MSG_ExpRwB
        DW      MSG_ExpRwF
        DW      MSG_ExpRwBF
        DW      MSG_ExpExp

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      06H             , 00H           ; 0:  coordStep->2
        DB      0eH             , 01H           ; 2:  tkMinus->5
        DB      01H                             ; 4:  Reject
        DB      07H             , 01H           ; 5:  coord2Step->8
        DB      01H                             ; 7:  Reject
        DB      0dH             , 01H           ; 8:  tkComma->11
        DB      00H                             ; 10:  Accept
        DB      08H             , 02H           ; 11:  Exp->15
        DB      04H             , 02H           ; 13:  empty->17
        DB      02H             , 01H           ; 15:  MARK(1)
        DB      0dH             , 01H           ; 17:  tkComma->20
        DB      00H                             ; 19:  Accept
        DB      0bH             , 0eH           ; 20:  RwBF->36
        DB      09H             , 02H           ; 22:  RwB->26
        DB      04H             , 0cH           ; 24:  empty->38
        DB      0aH             , 04H           ; 26:  RwF->32
        DB      02H             , 02H           ; 28:  MARK(2)
        DB      04H             , 06H           ; 30:  empty->38
        DB      02H             , 03H           ; 32:  MARK(3)
        DB      04H             , 02H           ; 34:  empty->38
        DB      02H             , 03H           ; 36:  MARK(3)
        DB      05H             , 01H           ; 38:  commaExp->41
        DB      00H                             ; 40:  Accept
        DB      02H             , 04H           ; 41:  MARK(4)
        DB      00H                             ; 43:  Accept

        DB      0cH             , 01H           ; 44:  fn1arg->47
        DB      01H                             ; 46:  Reject
        DB      03H                             ; 47:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 50:  Accept

        DB      0dH             , 01H           ; 51:  tkComma->54
        DB      01H                             ; 53:  Reject
        DB      08H             , 0FFH          ; 54:  Exp->Accept
        DB      01H                             ; 56:  Reject

; state table = 57 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
