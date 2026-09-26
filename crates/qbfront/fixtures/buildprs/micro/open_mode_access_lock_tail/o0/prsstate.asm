        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 17:10:41 2026


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
        EXTRN   NtoptFilenum:NEAR
        EXTRN   Ntexp12:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtExp
        DW      NtoptFilenum
        DW      Ntexp12

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      MSG_ExpExp
        DW      0       ; optFilenum
        DW      MSG_ExpExp

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      05H             , 01H           ; 0:  Exp->3
        DB      01H                             ; 2:  Reject
        DB      0fH             , 02H           ; 3:  tkFOR->7
        DB      04H             , 01dH          ; 5:  empty->36
        DB      0cH             , 019H          ; 7:  tkAPPEND->34
        DB      010H            , 013H          ; 9:  tkINPUT->30
        DB      014H            , 0dH           ; 11:  tkOUTPUT->26
        DB      015H            , 07H           ; 13:  tkRANDOM->22
        DB      0eH             , 01H           ; 15:  tkBINARY->18
        DB      01H                             ; 17:  Reject
        DB      02H             , 05H           ; 18:  MARK(5)
        DB      04H             , 0eH           ; 20:  empty->36
        DB      02H             , 04H           ; 22:  MARK(4)
        DB      04H             , 0aH           ; 24:  empty->36
        DB      02H             , 03H           ; 26:  MARK(3)
        DB      04H             , 06H           ; 28:  empty->36
        DB      02H             , 02H           ; 30:  MARK(2)
        DB      04H             , 02H           ; 32:  empty->36
        DB      02H             , 01H           ; 34:  MARK(1)
        DB      0bH             , 02H           ; 36:  tkACCESS->40
        DB      04H             , 011H          ; 38:  empty->57
        DB      016H            , 07H           ; 40:  tkREAD->49
        DB      018H            , 01H           ; 42:  tkWRITE->45
        DB      01H                             ; 44:  Reject
        DB      02H             , 07H           ; 45:  MARK(7)
        DB      04H             , 08H           ; 47:  empty->57
        DB      02H             , 06H           ; 49:  MARK(6)
        DB      018H            , 02H           ; 51:  tkWRITE->55
        DB      04H             , 02H           ; 53:  empty->57
        DB      02H             , 08H           ; 55:  MARK(8)
        DB      012H            , 08H           ; 57:  tkLOCK->67
        DB      017H            , 02H           ; 59:  tkSHARED->63
        DB      04H             , 015H          ; 61:  empty->84
        DB      02H             , 0cH           ; 63:  MARK(12)
        DB      04H             , 011H          ; 65:  empty->84
        DB      016H            , 07H           ; 67:  tkREAD->76
        DB      018H            , 01H           ; 69:  tkWRITE->72
        DB      01H                             ; 71:  Reject
        DB      02H             , 0aH           ; 72:  MARK(10)
        DB      04H             , 08H           ; 74:  empty->84
        DB      018H            , 04H           ; 76:  tkWRITE->82
        DB      02H             , 09H           ; 78:  MARK(9)
        DB      04H             , 02H           ; 80:  empty->84
        DB      02H             , 0bH           ; 82:  MARK(11)
        DB      0dH             , 0cH           ; 84:  tkAS->98
        DB      08H             , 01H           ; 86:  tkComma->89
        DB      01H                             ; 88:  Reject
        DB      06H             , 01H           ; 89:  optFilenum->92
        DB      01H                             ; 91:  Reject
        DB      07H             , 01H           ; 92:  exp12->95
        DB      01H                             ; 94:  Reject
        DB      02H             , 0eH           ; 95:  MARK(14)
        DB      00H                             ; 97:  Accept
        DB      06H             , 01H           ; 98:  optFilenum->101
        DB      01H                             ; 100:  Reject
        DB      011H            , 01H           ; 101:  tkLEN->104
        DB      00H                             ; 103:  Accept
        DB      09H             , 01H           ; 104:  tkEQ->107
        DB      01H                             ; 106:  Reject
        DB      05H             , 01H           ; 107:  Exp->110
        DB      01H                             ; 109:  Reject
        DB      02H             , 0dH           ; 110:  MARK(13)
        DB      00H                             ; 112:  Accept

        DB      05H             , 01H           ; 113:  Exp->116
        DB      01H                             ; 115:  Reject
        DB      03H                             ; 116:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 119:  Accept

; state table = 120 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
