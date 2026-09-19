        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 16:44:58 2026


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
        EXTRN   NtlbsInpExpComma:NEAR
        EXTRN   NtLitString:NEAR
        EXTRN   NtIdAryElemRef:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtlbsInpExpComma
        DW      NtLitString
        DW      NtIdAryElemRef

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      0       ; lbsInpExpComma
        DW      MSG_ExpExp
        DW      MSG_ExpVar

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      05H             , 022H          ; 0:  lbsInpExpComma->36
        DB      09H             , 0fH           ; 2:  tkSColon->19
        DB      06H             , 02H           ; 4:  LitString->8
        DB      04H             , 01eH          ; 6:  empty->38
        DB      02H             , 04H           ; 8:  MARK(4)
        DB      09H             , 01aH          ; 10:  tkSColon->38
        DB      08H             , 01H           ; 12:  tkComma->15
        DB      01H                             ; 14:  Reject
        DB      02H             , 01H           ; 15:  MARK(1)
        DB      04H             , 013H          ; 17:  empty->38
        DB      02H             , 02H           ; 19:  MARK(2)
        DB      06H             , 02H           ; 21:  LitString->25
        DB      04H             , 0dH           ; 23:  empty->38
        DB      02H             , 04H           ; 25:  MARK(4)
        DB      09H             , 09H           ; 27:  tkSColon->38
        DB      08H             , 01H           ; 29:  tkComma->32
        DB      01H                             ; 31:  Reject
        DB      02H             , 01H           ; 32:  MARK(1)
        DB      04H             , 02H           ; 34:  empty->38
        DB      02H             , 010H          ; 36:  MARK(16)
        DB      02H             , 08H           ; 38:  MARK(8)
        DB      07H             , 01H           ; 40:  IdAryElemRef->43
        DB      01H                             ; 42:  Reject
        DB      03H                             ; 43:  EMIT(opStInput)
        DW      opStInput                       
        DB      08H             , 04H           ; 46:  tkComma->52
        DB      03H                             ; 48:  EMIT(opInputEos)
        DW      opInputEos                      
        DB      00H                             ; 51:  Accept
        DB      07H             , 01H           ; 52:  IdAryElemRef->55
        DB      01H                             ; 54:  Reject
        DB      03H                             ; 55:  EMIT(opStInput)
        DW      opStInput                       
        DB      04H             , 0d2H          ; 58:  empty->46

        DB      06H             , 01H           ; 60:  LitString->63
        DB      01H                             ; 62:  Reject
        DB      03H                             ; 63:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 66:  Accept

; state table = 67 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
