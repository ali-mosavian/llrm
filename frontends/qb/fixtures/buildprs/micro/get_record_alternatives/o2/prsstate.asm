        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 16:15:50 2026


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
        EXTRN   NtIdAryElemRef:NEAR
        EXTRN   NtoptFilenum:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD
        DW      45      ; EMITFFFF


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtExp
        DW      NtIdAryElemRef
        DW      NtoptFilenum

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      MSG_ExpExp
        DW      MSG_ExpVar
        DW      0       ; optFilenum

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      08H             , 01H           ; 0:  optFilenum->3
        DB      01H                             ; 2:  Reject
        DB      09H             , 04H           ; 3:  tkComma->9
        DB      03H                             ; 5:  EMIT(opStGet1)
        DW      opStGet1                        
        DB      00H                             ; 8:  Accept
        DB      06H             , 0cH           ; 9:  Exp->23
        DB      09H             , 01H           ; 11:  tkComma->14
        DB      01H                             ; 13:  Reject
        DB      07H             , 01H           ; 14:  IdAryElemRef->17
        DB      01H                             ; 16:  Reject
        DB      03H                             ; 17:  EMIT(opStGetRec2)
        DW      opStGetRec2                     
        DB      05H             , 0FFH          ; 20:  EMITFFFF->Accept
        DB      01H                             ; 22:  Reject
        DB      09H             , 04H           ; 23:  tkComma->29
        DB      03H                             ; 25:  EMIT(opStGet2)
        DW      opStGet2                        
        DB      00H                             ; 28:  Accept
        DB      07H             , 01H           ; 29:  IdAryElemRef->32
        DB      01H                             ; 31:  Reject
        DB      03H                             ; 32:  EMIT(opStGetRec3)
        DW      opStGetRec3                     
        DB      05H             , 0FFH          ; 35:  EMITFFFF->Accept
        DB      01H                             ; 37:  Reject

        DB      06H             , 01H           ; 38:  Exp->41
        DB      01H                             ; 40:  Reject
        DB      03H                             ; 41:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 44:  Accept

        DB      03H                             ; 45:  EMIT(UNDEFINED)
        DW      UNDEFINED                       
        DB      00H                             ; 48:  Accept

; state table = 49 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
