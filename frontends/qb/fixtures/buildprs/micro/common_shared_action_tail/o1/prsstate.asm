        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 17:31:43 2026


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
        EXTRN   NtACTIONidCommon:NEAR
        EXTRN   NtACTIONidShared:NEAR
        EXTRN   NtExp:NEAR
        EXTRN   NtIdAryI:NEAR
        EXTRN   NtIdNamCom:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD
        DW      61      ; EMITFFFF


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtACTIONidCommon
        DW      NtACTIONidShared
        DW      NtExp
        DW      NtIdAryI
        DW      NtIdNamCom

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      0       ; ACTIONidCommon
        DW      0       ; ACTIONidShared
        DW      MSG_ExpExp
        DW      MSG_ExpVar
        DW      0       ; IdNamCom

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      0fH             , 011H          ; 0:  tkSHARED->19
        DB      03H                             ; 2:  EMIT(opStCommon)
        DW      opStCommon                      
        DB      05H             , 01H           ; 5:  EMITFFFF->8
        DB      01H                             ; 7:  Reject
        DB      0bH             , 03H           ; 8:  tkDiv->13
        DB      05H             , 01eH          ; 10:  EMITFFFF->42
        DB      01H                             ; 12:  Reject
        DB      0aH             , 01H           ; 13:  IdNamCom->16
        DB      01H                             ; 15:  Reject
        DB      0bH             , 018H          ; 16:  tkDiv->42
        DB      01H                             ; 18:  Reject
        DB      03H                             ; 19:  EMIT(opShared)
        DW      opShared                        
        DB      03H                             ; 22:  EMIT(opStCommon)
        DW      opStCommon                      
        DB      05H             , 01H           ; 25:  EMITFFFF->28
        DB      01H                             ; 27:  Reject
        DB      0bH             , 03H           ; 28:  tkDiv->33
        DB      05H             , 07H           ; 30:  EMITFFFF->39
        DB      01H                             ; 32:  Reject
        DB      0aH             , 01H           ; 33:  IdNamCom->36
        DB      01H                             ; 35:  Reject
        DB      0bH             , 01H           ; 36:  tkDiv->39
        DB      01H                             ; 38:  Reject
        DB      07H             , 01H           ; 39:  ACTIONidShared->42
        DB      01H                             ; 41:  Reject
        DB      06H             , 01H           ; 42:  ACTIONidCommon->45
        DB      01H                             ; 44:  Reject
        DB      09H             , 01H           ; 45:  IdAryI->48
        DB      01H                             ; 47:  Reject
        DB      0cH             , 01H           ; 48:  tkComma->51
        DB      00H                             ; 50:  Accept
        DB      09H             , 0dbH          ; 51:  IdAryI->48
        DB      01H                             ; 53:  Reject

        DB      08H             , 01H           ; 54:  Exp->57
        DB      01H                             ; 56:  Reject
        DB      03H                             ; 57:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 60:  Accept

        DB      03H                             ; 61:  EMIT(UNDEFINED)
        DW      UNDEFINED                       
        DB      00H                             ; 64:  Accept

; state table = 65 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
