        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 21:40:28 2026


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
        EXTRN   NtExp:NEAR
        EXTRN   NtStatementList:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD
        DW      116     ; EMITFFFF


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtErrIfNot1st
        DW      NtExp
        DW      NtStatementList

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      0       ; ErrIfNot1st
        DW      MSG_ExpExp
        DW      MSG_ExpStatement

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      018H            , 0fH           ; 0:  tkWHILE->17
        DB      017H            , 04H           ; 2:  tkUNTIL->8
        DB      03H                             ; 4:  EMIT(opStDo)
        DW      opStDo                          
        DB      00H                             ; 7:  Accept
        DB      07H             , 01H           ; 8:  Exp->11
        DB      01H                             ; 10:  Reject
        DB      03H                             ; 11:  EMIT(opStDoUntil)
        DW      opStDoUntil                     
        DB      05H             , 0FFH          ; 14:  EMITFFFF->Accept
        DB      01H                             ; 16:  Reject
        DB      07H             , 01H           ; 17:  Exp->20
        DB      01H                             ; 19:  Reject
        DB      03H                             ; 20:  EMIT(opStDoWhile)
        DW      opStDoWhile                     
        DB      04H             , 04bH          ; 23:  empty->100

        DB      07H             , 01H           ; 25:  Exp->28
        DB      01H                             ; 27:  Reject
        DB      03H                             ; 28:  EMIT(opStDraw)
        DW      opStDraw                        
        DB      00H                             ; 31:  Accept

        DB      07H             , 01H           ; 32:  Exp->35
        DB      01H                             ; 34:  Reject
        DB      015H            , 01H           ; 35:  tkTHEN->38
        DB      01H                             ; 37:  Reject
        DB      03H                             ; 38:  EMIT(opStElseIf)
        DW      opStElseIf                      
        DB      05H             , 01H           ; 41:  EMITFFFF->44
        DB      01H                             ; 43:  Reject
        DB      08H             , 0FFH          ; 44:  StatementList->Accept
        DB      00H                             ; 46:  Accept

        DB      03H                             ; 47:  EMIT(opStElse)
        DW      opStElse                        
        DB      05H             , 01H           ; 50:  EMITFFFF->53
        DB      01H                             ; 52:  Reject
        DB      08H             , 0FFH          ; 53:  StatementList->Accept
        DB      00H                             ; 55:  Accept

        DB      0aH             , 024H          ; 56:  tkDEF->94
        DB      011H            , 01eH          ; 58:  tkFUNCTION->90
        DB      012H            , 017H          ; 60:  tkIF->85
        DB      013H            , 011H          ; 62:  tkSELECT->81
        DB      014H            , 0bH           ; 64:  tkSUB->77
        DB      016H            , 04H           ; 66:  tkTYPE->72
        DB      03H                             ; 68:  EMIT(opStEnd)
        DW      opStEnd                         
        DB      00H                             ; 71:  Accept
        DB      03H                             ; 72:  EMIT(opStEndType)
        DW      opStEndType                     
        DB      04H             , 017H          ; 75:  empty->100
        DB      03H                             ; 77:  EMIT(opStEndProc)
        DW      opStEndProc                     
        DB      00H                             ; 80:  Accept
        DB      03H                             ; 81:  EMIT(opStEndSelect)
        DW      opStEndSelect                   
        DB      00H                             ; 84:  Accept
        DB      03H                             ; 85:  EMIT(opStEndIfBlock)
        DW      opStEndIfBlock                  
        DB      04H             , 010H          ; 88:  empty->106
        DB      03H                             ; 90:  EMIT(opStEndProc)
        DW      opStEndProc                     
        DB      00H                             ; 93:  Accept
        DB      03H                             ; 94:  EMIT(opStEndDef)
        DW      opStEndDef                      
        DB      03H                             ; 97:  EMIT(2)
        DW      2
        DB      05H             , 0FFH          ; 100:  EMITFFFF->Accept
        DB      01H                             ; 102:  Reject

        DB      03H                             ; 103:  EMIT(opStEndIfBlock)
        DW      opStEndIfBlock                  
        DB      06H             , 0FFH          ; 106:  ErrIfNot1st->Accept
        DB      01H                             ; 108:  Reject

        DB      07H             , 01H           ; 109:  Exp->112
        DB      01H                             ; 111:  Reject
        DB      03H                             ; 112:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 115:  Accept

        DB      03H                             ; 116:  EMIT(UNDEFINED)
        DW      UNDEFINED                       
        DB      00H                             ; 119:  Accept

; state table = 120 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
