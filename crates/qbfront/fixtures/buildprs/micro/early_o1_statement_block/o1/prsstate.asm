        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 17:41:14 2026


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
        EXTRN   NtCommaNoEos:NEAR
        EXTRN   NtcoordStep:NEAR
        EXTRN   NtExp:NEAR
        EXTRN   NtIdAryI:NEAR
        EXTRN   NtIdCallArg:NEAR
        EXTRN   NtIdNamCom:NEAR
        EXTRN   NtIdSubRef:NEAR
        EXTRN   NtNArgsMax3:NEAR
        EXTRN   NtoptFilenum:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD
        DW      289     ; caseItem
        DW      270     ; commaExp
        DW      248     ; evSwitch
        DW      267     ; expCommaExp
        DW      273     ; EMITFFFF
        DW      277     ; fn1arg
        DW      286     ; optCommaExp


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtACTIONidCommon
        DW      NtACTIONidShared
        DW      NtCommaNoEos
        DW      NtcoordStep
        DW      NtExp
        DW      NtIdAryI
        DW      NtIdCallArg
        DW      NtIdNamCom
        DW      NtIdSubRef
        DW      NtNArgsMax3
        DW      NtoptFilenum

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      0       ; ACTIONidCommon
        DW      0       ; ACTIONidShared
        DW      0       ; CommaNoEos
        DW      MSG_ExpCoord
        DW      MSG_ExpExp
        DW      MSG_ExpVar
        DW      MSG_ExpIdCallArg
        DW      0       ; IdNamCom
        DW      0       ; IdSubRef
        DW      MSG_ExpNArgs
        DW      0       ; optFilenum

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      03H                             ; 0:  EMIT(opStBeep)
        DW      opStBeep                        
        DB      00H                             ; 3:  Accept

        DB      010H            , 01H           ; 4:  Exp->7
        DB      01H                             ; 6:  Reject
        DB      0bH             , 0FFH          ; 7:  optCommaExp->Accept
        DB      01H                             ; 9:  Reject

        DB      08H             , 01H           ; 10:  expCommaExp->13
        DB      01H                             ; 12:  Reject
        DB      06H             , 01H           ; 13:  commaExp->16
        DB      01H                             ; 15:  Reject
        DB      03H                             ; 16:  EMIT(opStBsave)
        DW      opStBsave                       
        DB      00H                             ; 19:  Accept

        DB      02H             , 01H           ; 20:  MARK(1)
        DB      014H            , 01H           ; 22:  IdSubRef->25
        DB      01H                             ; 24:  Reject
        DB      018H            , 01H           ; 25:  tkLParen->28
        DB      00H                             ; 27:  Accept
        DB      012H            , 01H           ; 28:  IdCallArg->31
        DB      01H                             ; 30:  Reject
        DB      01aH            , 03H           ; 31:  tkComma->36
        DB      019H            , 0FFH          ; 33:  tkRParen->Accept
        DB      01H                             ; 35:  Reject
        DB      012H            , 0d9H          ; 36:  IdCallArg->31
        DB      01H                             ; 38:  Reject

        DB      02H             , 01H           ; 39:  MARK(1)
        DB      014H            , 01H           ; 41:  IdSubRef->44
        DB      01H                             ; 43:  Reject
        DB      018H            , 01H           ; 44:  tkLParen->47
        DB      00H                             ; 46:  Accept
        DB      010H            , 01H           ; 47:  Exp->50
        DB      01H                             ; 49:  Reject
        DB      01aH            , 03H           ; 50:  tkComma->55
        DB      019H            , 0FFH          ; 52:  tkRParen->Accept
        DB      01H                             ; 54:  Reject
        DB      010H            , 0d9H          ; 55:  Exp->50
        DB      01H                             ; 57:  Reject

        DB      02bH            , 09H           ; 58:  tkELSE->69
        DB      05H             , 01H           ; 60:  caseItem->63
        DB      01H                             ; 62:  Reject
        DB      01aH            , 01H           ; 63:  tkComma->66
        DB      00H                             ; 65:  Accept
        DB      05H             , 0dbH          ; 66:  caseItem->63
        DB      01H                             ; 68:  Reject
        DB      03H                             ; 69:  EMIT(opStCaseElse)
        DW      opStCaseElse                    
        DB      00H                             ; 72:  Accept

        DB      010H            , 01H           ; 73:  Exp->76
        DB      01H                             ; 75:  Reject
        DB      03H                             ; 76:  EMIT(opStChain)
        DW      opStChain                       
        DB      00H                             ; 79:  Accept

        DB      010H            , 01H           ; 80:  Exp->83
        DB      01H                             ; 82:  Reject
        DB      03H                             ; 83:  EMIT(opStChdir)
        DW      opStChdir                       
        DB      00H                             ; 86:  Accept

        DB      0fH             , 01H           ; 87:  coordStep->90
        DB      01H                             ; 89:  Reject
        DB      06H             , 01H           ; 90:  commaExp->93
        DB      01H                             ; 92:  Reject
        DB      01aH            , 01H           ; 93:  tkComma->96
        DB      00H                             ; 95:  Accept
        DB      010H            , 02H           ; 96:  Exp->100
        DB      04H             , 02H           ; 98:  empty->102
        DB      02H             , 01H           ; 100:  MARK(1)
        DB      0eH             , 01H           ; 102:  CommaNoEos->105
        DB      00H                             ; 104:  Accept
        DB      010H            , 03H           ; 105:  Exp->110
        DB      03H                             ; 107:  EMIT(opNull)
        DW      opNull                          
        DB      03H                             ; 110:  EMIT(opCircleStart)
        DW      opCircleStart                   
        DB      0eH             , 01H           ; 113:  CommaNoEos->116
        DB      00H                             ; 115:  Accept
        DB      010H            , 02H           ; 116:  Exp->120
        DB      04H             , 03H           ; 118:  empty->123
        DB      03H                             ; 120:  EMIT(opCircleEnd)
        DW      opCircleEnd                     
        DB      06H             , 01H           ; 123:  commaExp->126
        DB      00H                             ; 125:  Accept
        DB      03H                             ; 126:  EMIT(opCircleAspect)
        DW      opCircleAspect                  
        DB      00H                             ; 129:  Accept

        DB      015H            , 0FFH          ; 130:  NArgsMax3->Accept
        DB      01H                             ; 132:  Reject

        DB      016H            , 01H           ; 133:  optFilenum->136
        DB      00H                             ; 135:  Accept
        DB      01aH            , 01H           ; 136:  tkComma->139
        DB      00H                             ; 138:  Accept
        DB      016H            , 0dbH          ; 139:  optFilenum->136
        DB      01H                             ; 141:  Reject

        DB      010H            , 03H           ; 142:  Exp->147
        DB      03H                             ; 144:  EMIT(opUndef)
        DW      opUndef                         
        DB      03H                             ; 147:  EMIT(opStCls)
        DW      opStCls                         
        DB      00H                             ; 150:  Accept

        DB      0aH             , 01H           ; 151:  fn1arg->154
        DB      01H                             ; 153:  Reject
        DB      03H                             ; 154:  EMIT(opEvCom)
        DW      opEvCom                         
        DB      04H             , 04fH          ; 157:  empty->238

        DB      030H            , 011H          ; 159:  tkSHARED->178
        DB      03H                             ; 161:  EMIT(opStCommon)
        DW      opStCommon                      
        DB      09H             , 01H           ; 164:  EMITFFFF->167
        DB      01H                             ; 166:  Reject
        DB      017H            , 03H           ; 167:  tkDiv->172
        DB      09H             , 01eH          ; 169:  EMITFFFF->201
        DB      01H                             ; 171:  Reject
        DB      013H            , 01H           ; 172:  IdNamCom->175
        DB      01H                             ; 174:  Reject
        DB      017H            , 018H          ; 175:  tkDiv->201
        DB      01H                             ; 177:  Reject
        DB      03H                             ; 178:  EMIT(opShared)
        DW      opShared                        
        DB      03H                             ; 181:  EMIT(opStCommon)
        DW      opStCommon                      
        DB      09H             , 01H           ; 184:  EMITFFFF->187
        DB      01H                             ; 186:  Reject
        DB      017H            , 03H           ; 187:  tkDiv->192
        DB      09H             , 07H           ; 189:  EMITFFFF->198
        DB      01H                             ; 191:  Reject
        DB      013H            , 01H           ; 192:  IdNamCom->195
        DB      01H                             ; 194:  Reject
        DB      017H            , 01H           ; 195:  tkDiv->198
        DB      01H                             ; 197:  Reject
        DB      0dH             , 01H           ; 198:  ACTIONidShared->201
        DB      01H                             ; 200:  Reject
        DB      0cH             , 01H           ; 201:  ACTIONidCommon->204
        DB      01H                             ; 203:  Reject
        DB      011H            , 01H           ; 204:  IdAryI->207
        DB      01H                             ; 206:  Reject
        DB      01aH            , 01H           ; 207:  tkComma->210
        DB      00H                             ; 209:  Accept
        DB      011H            , 0dbH          ; 210:  IdAryI->207
        DB      01H                             ; 212:  Reject

        DB      03H                             ; 213:  EMIT(opEvPen)
        DW      opEvPen                         
        DB      04H             , 014H          ; 216:  empty->238

        DB      010H            , 01H           ; 218:  Exp->221
        DB      01H                             ; 220:  Reject
        DB      03H                             ; 221:  EMIT(opStPlay)
        DW      opStPlay                        
        DB      00H                             ; 224:  Accept

        DB      03H                             ; 225:  EMIT(opEvPlay0)
        DW      opEvPlay0                       
        DB      04H             , 08H           ; 228:  empty->238

        DB      03H                             ; 230:  EMIT(opEvTimer0)
        DW      opEvTimer0                      
        DB      04H             , 03H           ; 233:  empty->238

        DB      03H                             ; 235:  EMIT(opEvUEvent)
        DW      opEvUEvent                      
        DB      07H             , 0FFH          ; 238:  evSwitch->Accept
        DB      01H                             ; 240:  Reject

        DB      010H            , 01H           ; 241:  Exp->244
        DB      01H                             ; 243:  Reject
        DB      03H                             ; 244:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 247:  Accept

        DB      02dH            , 0dH           ; 248:  tkON->263
        DB      02cH            , 07H           ; 250:  tkOFF->259
        DB      031H            , 01H           ; 252:  tkSTOP->255
        DB      01H                             ; 254:  Reject
        DB      03H                             ; 255:  EMIT(opEvStop)
        DW      opEvStop                        
        DB      00H                             ; 258:  Accept
        DB      03H                             ; 259:  EMIT(opEvOff)
        DW      opEvOff                         
        DB      00H                             ; 262:  Accept
        DB      03H                             ; 263:  EMIT(opEvOn)
        DW      opEvOn                          
        DB      00H                             ; 266:  Accept

        DB      010H            , 01H           ; 267:  Exp->270
        DB      01H                             ; 269:  Reject

        DB      01aH            , 011H          ; 270:  tkComma->289
        DB      01H                             ; 272:  Reject

        DB      03H                             ; 273:  EMIT(UNDEFINED)
        DW      UNDEFINED                       
        DB      00H                             ; 276:  Accept

        DB      018H            , 01H           ; 277:  tkLParen->280
        DB      01H                             ; 279:  Reject
        DB      010H            , 01H           ; 280:  Exp->283
        DB      01H                             ; 282:  Reject
        DB      019H            , 0FFH          ; 283:  tkRParen->Accept
        DB      01H                             ; 285:  Reject

        DB      01aH            , 01H           ; 286:  tkComma->289
        DB      00H                             ; 288:  Accept

        DB      010H            , 0FFH          ; 289:  Exp->Accept
        DB      01H                             ; 291:  Reject

; state table = 292 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
