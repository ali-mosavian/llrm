        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 17:41:16 2026


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
        DW      272     ; caseItem
        DW      253     ; commaExp
        DW      231     ; evSwitch
        DW      250     ; expCommaExp
        DW      256     ; EMITFFFF
        DW      260     ; fn1arg
        DW      269     ; optCommaExp


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
        DB      012H            , 0cH           ; 28:  IdCallArg->42
        DB      01H                             ; 30:  Reject

        DB      02H             , 01H           ; 31:  MARK(1)
        DB      014H            , 01H           ; 33:  IdSubRef->36
        DB      01H                             ; 35:  Reject
        DB      018H            , 01H           ; 36:  tkLParen->39
        DB      00H                             ; 38:  Accept
        DB      010H            , 01H           ; 39:  Exp->42
        DB      01H                             ; 41:  Reject
        DB      01aH            , 0d0H          ; 42:  tkComma->28
        DB      019H            , 0FFH          ; 44:  tkRParen->Accept
        DB      01H                             ; 46:  Reject

        DB      02bH            , 06H           ; 47:  tkELSE->55
        DB      05H             , 01H           ; 49:  caseItem->52
        DB      01H                             ; 51:  Reject
        DB      01aH            , 0dbH          ; 52:  tkComma->49
        DB      00H                             ; 54:  Accept
        DB      03H                             ; 55:  EMIT(opStCaseElse)
        DW      opStCaseElse                    
        DB      00H                             ; 58:  Accept

        DB      010H            , 01H           ; 59:  Exp->62
        DB      01H                             ; 61:  Reject
        DB      03H                             ; 62:  EMIT(opStChain)
        DW      opStChain                       
        DB      00H                             ; 65:  Accept

        DB      010H            , 01H           ; 66:  Exp->69
        DB      01H                             ; 68:  Reject
        DB      03H                             ; 69:  EMIT(opStChdir)
        DW      opStChdir                       
        DB      00H                             ; 72:  Accept

        DB      0fH             , 01H           ; 73:  coordStep->76
        DB      01H                             ; 75:  Reject
        DB      06H             , 01H           ; 76:  commaExp->79
        DB      01H                             ; 78:  Reject
        DB      01aH            , 01H           ; 79:  tkComma->82
        DB      00H                             ; 81:  Accept
        DB      010H            , 02H           ; 82:  Exp->86
        DB      04H             , 02H           ; 84:  empty->88
        DB      02H             , 01H           ; 86:  MARK(1)
        DB      0eH             , 01H           ; 88:  CommaNoEos->91
        DB      00H                             ; 90:  Accept
        DB      010H            , 03H           ; 91:  Exp->96
        DB      03H                             ; 93:  EMIT(opNull)
        DW      opNull                          
        DB      03H                             ; 96:  EMIT(opCircleStart)
        DW      opCircleStart                   
        DB      0eH             , 01H           ; 99:  CommaNoEos->102
        DB      00H                             ; 101:  Accept
        DB      010H            , 02H           ; 102:  Exp->106
        DB      04H             , 03H           ; 104:  empty->109
        DB      03H                             ; 106:  EMIT(opCircleEnd)
        DW      opCircleEnd                     
        DB      06H             , 01H           ; 109:  commaExp->112
        DB      00H                             ; 111:  Accept
        DB      03H                             ; 112:  EMIT(opCircleAspect)
        DW      opCircleAspect                  
        DB      00H                             ; 115:  Accept

        DB      015H            , 0FFH          ; 116:  NArgsMax3->Accept
        DB      01H                             ; 118:  Reject

        DB      016H            , 01H           ; 119:  optFilenum->122
        DB      00H                             ; 121:  Accept
        DB      01aH            , 01H           ; 122:  tkComma->125
        DB      00H                             ; 124:  Accept
        DB      016H            , 0dbH          ; 125:  optFilenum->122
        DB      01H                             ; 127:  Reject

        DB      010H            , 03H           ; 128:  Exp->133
        DB      03H                             ; 130:  EMIT(opUndef)
        DW      opUndef                         
        DB      03H                             ; 133:  EMIT(opStCls)
        DW      opStCls                         
        DB      00H                             ; 136:  Accept

        DB      0aH             , 01H           ; 137:  fn1arg->140
        DB      01H                             ; 139:  Reject
        DB      03H                             ; 140:  EMIT(opEvCom)
        DW      opEvCom                         
        DB      04H             , 04cH          ; 143:  empty->221

        DB      030H            , 011H          ; 145:  tkSHARED->164
        DB      03H                             ; 147:  EMIT(opStCommon)
        DW      opStCommon                      
        DB      09H             , 01H           ; 150:  EMITFFFF->153
        DB      01H                             ; 152:  Reject
        DB      017H            , 03H           ; 153:  tkDiv->158
        DB      09H             , 01eH          ; 155:  EMITFFFF->187
        DB      01H                             ; 157:  Reject
        DB      013H            , 01H           ; 158:  IdNamCom->161
        DB      01H                             ; 160:  Reject
        DB      017H            , 018H          ; 161:  tkDiv->187
        DB      01H                             ; 163:  Reject
        DB      03H                             ; 164:  EMIT(opShared)
        DW      opShared                        
        DB      03H                             ; 167:  EMIT(opStCommon)
        DW      opStCommon                      
        DB      09H             , 01H           ; 170:  EMITFFFF->173
        DB      01H                             ; 172:  Reject
        DB      017H            , 03H           ; 173:  tkDiv->178
        DB      09H             , 07H           ; 175:  EMITFFFF->184
        DB      01H                             ; 177:  Reject
        DB      013H            , 01H           ; 178:  IdNamCom->181
        DB      01H                             ; 180:  Reject
        DB      017H            , 01H           ; 181:  tkDiv->184
        DB      01H                             ; 183:  Reject
        DB      0dH             , 01H           ; 184:  ACTIONidShared->187
        DB      01H                             ; 186:  Reject
        DB      0cH             , 01H           ; 187:  ACTIONidCommon->190
        DB      01H                             ; 189:  Reject
        DB      011H            , 01H           ; 190:  IdAryI->193
        DB      01H                             ; 192:  Reject
        DB      01aH            , 0dbH          ; 193:  tkComma->190
        DB      00H                             ; 195:  Accept

        DB      03H                             ; 196:  EMIT(opEvPen)
        DW      opEvPen                         
        DB      04H             , 014H          ; 199:  empty->221

        DB      010H            , 01H           ; 201:  Exp->204
        DB      01H                             ; 203:  Reject
        DB      03H                             ; 204:  EMIT(opStPlay)
        DW      opStPlay                        
        DB      00H                             ; 207:  Accept

        DB      03H                             ; 208:  EMIT(opEvPlay0)
        DW      opEvPlay0                       
        DB      04H             , 08H           ; 211:  empty->221

        DB      03H                             ; 213:  EMIT(opEvTimer0)
        DW      opEvTimer0                      
        DB      04H             , 03H           ; 216:  empty->221

        DB      03H                             ; 218:  EMIT(opEvUEvent)
        DW      opEvUEvent                      
        DB      07H             , 0FFH          ; 221:  evSwitch->Accept
        DB      01H                             ; 223:  Reject

        DB      010H            , 01H           ; 224:  Exp->227
        DB      01H                             ; 226:  Reject
        DB      03H                             ; 227:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 230:  Accept

        DB      02dH            , 0dH           ; 231:  tkON->246
        DB      02cH            , 07H           ; 233:  tkOFF->242
        DB      031H            , 01H           ; 235:  tkSTOP->238
        DB      01H                             ; 237:  Reject
        DB      03H                             ; 238:  EMIT(opEvStop)
        DW      opEvStop                        
        DB      00H                             ; 241:  Accept
        DB      03H                             ; 242:  EMIT(opEvOff)
        DW      opEvOff                         
        DB      00H                             ; 245:  Accept
        DB      03H                             ; 246:  EMIT(opEvOn)
        DW      opEvOn                          
        DB      00H                             ; 249:  Accept

        DB      010H            , 01H           ; 250:  Exp->253
        DB      01H                             ; 252:  Reject

        DB      01aH            , 011H          ; 253:  tkComma->272
        DB      01H                             ; 255:  Reject

        DB      03H                             ; 256:  EMIT(UNDEFINED)
        DW      UNDEFINED                       
        DB      00H                             ; 259:  Accept

        DB      018H            , 01H           ; 260:  tkLParen->263
        DB      01H                             ; 262:  Reject
        DB      010H            , 01H           ; 263:  Exp->266
        DB      01H                             ; 265:  Reject
        DB      019H            , 0FFH          ; 266:  tkRParen->Accept
        DB      01H                             ; 268:  Reject

        DB      01aH            , 01H           ; 269:  tkComma->272
        DB      00H                             ; 271:  Accept

        DB      010H            , 0FFH          ; 272:  Exp->Accept
        DB      01H                             ; 274:  Reject

; state table = 275 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
