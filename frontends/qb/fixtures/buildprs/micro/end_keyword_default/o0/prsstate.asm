        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 16:10:09 2026


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
        EXTRN   NtErrIfNot1st:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD
        DW      53      ; EMITFFFF


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtErrIfNot1st

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      0       ; ErrIfNot1st

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      08H             , 026H          ; 0:  tkDEF->40
        DB      0aH             , 020H          ; 2:  tkFUNCTION->36
        DB      0bH             , 018H          ; 4:  tkIF->30
        DB      0cH             , 012H          ; 6:  tkSELECT->26
        DB      0dH             , 0cH           ; 8:  tkSUB->22
        DB      0eH             , 04H           ; 10:  tkTYPE->16
        DB      03H                             ; 12:  EMIT(opStEnd)
        DW      opStEnd                         
        DB      00H                             ; 15:  Accept
        DB      03H                             ; 16:  EMIT(opStEndType)
        DW      opStEndType                     
        DB      05H             , 0FFH          ; 19:  EMITFFFF->Accept
        DB      01H                             ; 21:  Reject
        DB      03H                             ; 22:  EMIT(opStEndProc)
        DW      opStEndProc                     
        DB      00H                             ; 25:  Accept
        DB      03H                             ; 26:  EMIT(opStEndSelect)
        DW      opStEndSelect                   
        DB      00H                             ; 29:  Accept
        DB      03H                             ; 30:  EMIT(opStEndIfBlock)
        DW      opStEndIfBlock                  
        DB      06H             , 0FFH          ; 33:  ErrIfNot1st->Accept
        DB      01H                             ; 35:  Reject
        DB      03H                             ; 36:  EMIT(opStEndProc)
        DW      opStEndProc                     
        DB      00H                             ; 39:  Accept
        DB      03H                             ; 40:  EMIT(opStEndDef)
        DW      opStEndDef                      
        DB      03H                             ; 43:  EMIT(2)
        DW      2
        DB      05H             , 0FFH          ; 46:  EMITFFFF->Accept
        DB      01H                             ; 48:  Reject

        DB      03H                             ; 49:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 52:  Accept

        DB      03H                             ; 53:  EMIT(UNDEFINED)
        DW      UNDEFINED                       
        DB      00H                             ; 56:  Accept

; state table = 57 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
