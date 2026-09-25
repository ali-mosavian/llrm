        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 21:40:26 2026


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
        DW      119     ; EMITFFFF


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
        DB      05H             , 0FFH          ; 23:  EMITFFFF->Accept
        DB      01H                             ; 25:  Reject

        DB      07H             , 01H           ; 26:  Exp->29
        DB      01H                             ; 28:  Reject
        DB      03H                             ; 29:  EMIT(opStDraw)
        DW      opStDraw                        
        DB      00H                             ; 32:  Accept

        DB      07H             , 01H           ; 33:  Exp->36
        DB      01H                             ; 35:  Reject
        DB      015H            , 01H           ; 36:  tkTHEN->39
        DB      01H                             ; 38:  Reject
        DB      03H                             ; 39:  EMIT(opStElseIf)
        DW      opStElseIf                      
        DB      05H             , 01H           ; 42:  EMITFFFF->45
        DB      01H                             ; 44:  Reject
        DB      08H             , 0FFH          ; 45:  StatementList->Accept
        DB      00H                             ; 47:  Accept

        DB      03H                             ; 48:  EMIT(opStElse)
        DW      opStElse                        
        DB      05H             , 01H           ; 51:  EMITFFFF->54
        DB      01H                             ; 53:  Reject
        DB      08H             , 0FFH          ; 54:  StatementList->Accept
        DB      00H                             ; 56:  Accept

        DB      0aH             , 026H          ; 57:  tkDEF->97
        DB      011H            , 020H          ; 59:  tkFUNCTION->93
        DB      012H            , 018H          ; 61:  tkIF->87
        DB      013H            , 012H          ; 63:  tkSELECT->83
        DB      014H            , 0cH           ; 65:  tkSUB->79
        DB      016H            , 04H           ; 67:  tkTYPE->73
        DB      03H                             ; 69:  EMIT(opStEnd)
        DW      opStEnd                         
        DB      00H                             ; 72:  Accept
        DB      03H                             ; 73:  EMIT(opStEndType)
        DW      opStEndType                     
        DB      05H             , 0FFH          ; 76:  EMITFFFF->Accept
        DB      01H                             ; 78:  Reject
        DB      03H                             ; 79:  EMIT(opStEndProc)
        DW      opStEndProc                     
        DB      00H                             ; 82:  Accept
        DB      03H                             ; 83:  EMIT(opStEndSelect)
        DW      opStEndSelect                   
        DB      00H                             ; 86:  Accept
        DB      03H                             ; 87:  EMIT(opStEndIfBlock)
        DW      opStEndIfBlock                  
        DB      06H             , 0FFH          ; 90:  ErrIfNot1st->Accept
        DB      01H                             ; 92:  Reject
        DB      03H                             ; 93:  EMIT(opStEndProc)
        DW      opStEndProc                     
        DB      00H                             ; 96:  Accept
        DB      03H                             ; 97:  EMIT(opStEndDef)
        DW      opStEndDef                      
        DB      03H                             ; 100:  EMIT(2)
        DW      2
        DB      05H             , 0FFH          ; 103:  EMITFFFF->Accept
        DB      01H                             ; 105:  Reject

        DB      03H                             ; 106:  EMIT(opStEndIfBlock)
        DW      opStEndIfBlock                  
        DB      06H             , 0FFH          ; 109:  ErrIfNot1st->Accept
        DB      01H                             ; 111:  Reject

        DB      07H             , 01H           ; 112:  Exp->115
        DB      01H                             ; 114:  Reject
        DB      03H                             ; 115:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 118:  Accept

        DB      03H                             ; 119:  EMIT(UNDEFINED)
        DW      UNDEFINED                       
        DB      00H                             ; 122:  Accept

; state table = 123 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
