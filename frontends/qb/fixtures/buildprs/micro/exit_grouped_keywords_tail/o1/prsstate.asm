        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 16:11:24 2026


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
        DW      31      ; EMITFFFF


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      07H             , 013H          ; 0:  tkDEF->21
        DB      0bH             , 011H          ; 2:  tkFUNCTION->21
        DB      0cH             , 0fH           ; 4:  tkSUB->21
        DB      08H             , 08H           ; 6:  tkDO->16
        DB      0aH             , 01H           ; 8:  tkFOR->11
        DB      01H                             ; 10:  Reject
        DB      03H                             ; 11:  EMIT(opStExitFor)
        DW      opStExitFor                     
        DB      04H             , 08H           ; 14:  empty->24
        DB      03H                             ; 16:  EMIT(opStExitDo)
        DW      opStExitDo                      
        DB      04H             , 03H           ; 19:  empty->24
        DB      03H                             ; 21:  EMIT(opStExitProc)
        DW      opStExitProc                    
        DB      05H             , 0FFH          ; 24:  EMITFFFF->Accept
        DB      01H                             ; 26:  Reject

        DB      03H                             ; 27:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 30:  Accept

        DB      03H                             ; 31:  EMIT(UNDEFINED)
        DW      UNDEFINED                       
        DB      00H                             ; 34:  Accept

; state table = 35 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
