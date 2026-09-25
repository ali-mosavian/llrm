        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 17:57:14 2026


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
        EXTRN   NtConstAssign:NEAR
        EXTRN   NtcoordStep:NEAR
        EXTRN   NtDeflistI2:NEAR
        EXTRN   NtDeflistI4:NEAR
        EXTRN   NtDeflistR4:NEAR
        EXTRN   NtDeflistR8:NEAR
        EXTRN   NtDeflistSD:NEAR
        EXTRN   NtExp:NEAR
        EXTRN   NtIdAryDim:NEAR
        EXTRN   NtIdAryI:NEAR
        EXTRN   NtIdCallArg:NEAR
        EXTRN   NtIdFn:NEAR
        EXTRN   NtIdFuncDecl:NEAR
        EXTRN   NtIdNamCom:NEAR
        EXTRN   NtIdSubDecl:NEAR
        EXTRN   NtIdSubRef:NEAR
        EXTRN   NtNArgsMax3:NEAR
        EXTRN   NtoptFilenum:NEAR
        EXTRN   Ntparms:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD
        DW      394     ; caseItem
        DW      375     ; commaExp
        DW      353     ; evSwitch
        DW      372     ; expCommaExp
        DW      378     ; EMITFFFF
        DW      382     ; fn1arg
        DW      391     ; optCommaExp


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtACTIONidCommon
        DW      NtACTIONidShared
        DW      NtCommaNoEos
        DW      NtConstAssign
        DW      NtcoordStep
        DW      NtDeflistI2
        DW      NtDeflistI4
        DW      NtDeflistR4
        DW      NtDeflistR8
        DW      NtDeflistSD
        DW      NtExp
        DW      NtIdAryDim
        DW      NtIdAryI
        DW      NtIdCallArg
        DW      NtIdFn
        DW      NtIdFuncDecl
        DW      NtIdNamCom
        DW      NtIdSubDecl
        DW      NtIdSubRef
        DW      NtNArgsMax3
        DW      NtoptFilenum
        DW      Ntparms

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      0       ; ACTIONidCommon
        DW      0       ; ACTIONidShared
        DW      0       ; CommaNoEos
        DW      0       ; ConstAssign
        DW      MSG_ExpCoord
        DW      0       ; DeflistI2
        DW      0       ; DeflistI4
        DW      0       ; DeflistR4
        DW      0       ; DeflistR8
        DW      0       ; DeflistSD
        DW      MSG_ExpExp
        DW      MSG_ExpVar
        DW      MSG_ExpVar
        DW      MSG_ExpIdCallArg
        DW      0       ; IdFn
        DW      0       ; IdFuncDecl
        DW      0       ; IdNamCom
        DW      0       ; IdSubDecl
        DW      0       ; IdSubRef
        DW      MSG_ExpNArgs
        DW      0       ; optFilenum
        DW      0       ; parms

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      03H                             ; 0:  EMIT(opStBeep)
        DW      opStBeep                        
        DB      00H                             ; 3:  Accept

        DB      016H            , 01H           ; 4:  Exp->7
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
        DB      01eH            , 01H           ; 22:  IdSubRef->25
        DB      01H                             ; 24:  Reject
        DB      024H            , 01H           ; 25:  tkLParen->28
        DB      00H                             ; 27:  Accept
        DB      019H            , 0cH           ; 28:  IdCallArg->42
        DB      01H                             ; 30:  Reject

        DB      02H             , 01H           ; 31:  MARK(1)
        DB      01eH            , 01H           ; 33:  IdSubRef->36
        DB      01H                             ; 35:  Reject
        DB      024H            , 01H           ; 36:  tkLParen->39
        DB      00H                             ; 38:  Accept
        DB      016H            , 01H           ; 39:  Exp->42
        DB      01H                             ; 41:  Reject
        DB      026H            , 0d0H          ; 42:  tkComma->28
        DB      025H            , 0FFH          ; 44:  tkRParen->Accept
        DB      01H                             ; 46:  Reject

        DB      042H            , 06H           ; 47:  tkELSE->55
        DB      05H             , 01H           ; 49:  caseItem->52
        DB      01H                             ; 51:  Reject
        DB      026H            , 0dbH          ; 52:  tkComma->49
        DB      00H                             ; 54:  Accept
        DB      03H                             ; 55:  EMIT(opStCaseElse)
        DW      opStCaseElse                    
        DB      00H                             ; 58:  Accept

        DB      016H            , 01H           ; 59:  Exp->62
        DB      01H                             ; 61:  Reject
        DB      03H                             ; 62:  EMIT(opStChain)
        DW      opStChain                       
        DB      00H                             ; 65:  Accept

        DB      016H            , 01H           ; 66:  Exp->69
        DB      01H                             ; 68:  Reject
        DB      03H                             ; 69:  EMIT(opStChdir)
        DW      opStChdir                       
        DB      00H                             ; 72:  Accept

        DB      010H            , 01H           ; 73:  coordStep->76
        DB      01H                             ; 75:  Reject
        DB      06H             , 01H           ; 76:  commaExp->79
        DB      01H                             ; 78:  Reject
        DB      026H            , 01H           ; 79:  tkComma->82
        DB      00H                             ; 81:  Accept
        DB      016H            , 02H           ; 82:  Exp->86
        DB      04H             , 02H           ; 84:  empty->88
        DB      02H             , 01H           ; 86:  MARK(1)
        DB      0eH             , 01H           ; 88:  CommaNoEos->91
        DB      00H                             ; 90:  Accept
        DB      016H            , 03H           ; 91:  Exp->96
        DB      03H                             ; 93:  EMIT(opNull)
        DW      opNull                          
        DB      03H                             ; 96:  EMIT(opCircleStart)
        DW      opCircleStart                   
        DB      0eH             , 01H           ; 99:  CommaNoEos->102
        DB      00H                             ; 101:  Accept
        DB      016H            , 02H           ; 102:  Exp->106
        DB      04H             , 03H           ; 104:  empty->109
        DB      03H                             ; 106:  EMIT(opCircleEnd)
        DW      opCircleEnd                     
        DB      06H             , 01H           ; 109:  commaExp->112
        DB      00H                             ; 111:  Accept
        DB      03H                             ; 112:  EMIT(opCircleAspect)
        DW      opCircleAspect                  
        DB      00H                             ; 115:  Accept

        DB      01fH            , 0FFH          ; 116:  NArgsMax3->Accept
        DB      01H                             ; 118:  Reject

        DB      020H            , 01H           ; 119:  optFilenum->122
        DB      00H                             ; 121:  Accept
        DB      026H            , 01H           ; 122:  tkComma->125
        DB      00H                             ; 124:  Accept
        DB      020H            , 0dbH          ; 125:  optFilenum->122
        DB      01H                             ; 127:  Reject

        DB      016H            , 03H           ; 128:  Exp->133
        DB      03H                             ; 130:  EMIT(opUndef)
        DW      opUndef                         
        DB      03H                             ; 133:  EMIT(opStCls)
        DW      opStCls                         
        DB      00H                             ; 136:  Accept

        DB      0aH             , 01H           ; 137:  fn1arg->140
        DB      01H                             ; 139:  Reject
        DB      03H                             ; 140:  EMIT(opEvCom)
        DW      opEvCom                         
        DB      04H             , 0e1H, 057H    ; 143:  empty->343

        DB      049H            , 011H          ; 146:  tkSHARED->165
        DB      03H                             ; 148:  EMIT(opStCommon)
        DW      opStCommon                      
        DB      09H             , 01H           ; 151:  EMITFFFF->154
        DB      01H                             ; 153:  Reject
        DB      023H            , 03H           ; 154:  tkDiv->159
        DB      09H             , 01eH          ; 156:  EMITFFFF->188
        DB      01H                             ; 158:  Reject
        DB      01cH            , 01H           ; 159:  IdNamCom->162
        DB      01H                             ; 161:  Reject
        DB      023H            , 018H          ; 162:  tkDiv->188
        DB      01H                             ; 164:  Reject
        DB      03H                             ; 165:  EMIT(opShared)
        DW      opShared                        
        DB      03H                             ; 168:  EMIT(opStCommon)
        DW      opStCommon                      
        DB      09H             , 01H           ; 171:  EMITFFFF->174
        DB      01H                             ; 173:  Reject
        DB      023H            , 03H           ; 174:  tkDiv->179
        DB      09H             , 07H           ; 176:  EMITFFFF->185
        DB      01H                             ; 178:  Reject
        DB      01cH            , 01H           ; 179:  IdNamCom->182
        DB      01H                             ; 181:  Reject
        DB      023H            , 01H           ; 182:  tkDiv->185
        DB      01H                             ; 184:  Reject
        DB      0dH             , 01H           ; 185:  ACTIONidShared->188
        DB      01H                             ; 187:  Reject
        DB      0cH             , 01H           ; 188:  ACTIONidCommon->191
        DB      01H                             ; 190:  Reject
        DB      018H            , 01H           ; 191:  IdAryI->194
        DB      01H                             ; 193:  Reject
        DB      026H            , 0dbH          ; 194:  tkComma->191
        DB      00H                             ; 196:  Accept

        DB      03H                             ; 197:  EMIT(opStConst)
        DW      opStConst                       
        DB      0fH             , 01H           ; 200:  ConstAssign->203
        DB      01H                             ; 202:  Reject
        DB      026H            , 0dbH          ; 203:  tkComma->200
        DB      00H                             ; 205:  Accept

        DB      022H            , 01H           ; 206:  tkEQ->209
        DB      01H                             ; 208:  Reject
        DB      016H            , 01H           ; 209:  Exp->212
        DB      01H                             ; 211:  Reject
        DB      03H                             ; 212:  EMIT(opStDate_)
        DW      opStDate_                       
        DB      00H                             ; 215:  Accept

        DB      043H            , 06H           ; 216:  tkFUNCTION->224
        DB      04bH            , 01H           ; 218:  tkSUB->221
        DB      01H                             ; 220:  Reject
        DB      01dH            , 04H           ; 221:  IdSubDecl->227
        DB      01H                             ; 223:  Reject
        DB      01bH            , 01H           ; 224:  IdFuncDecl->227
        DB      01H                             ; 226:  Reject
        DB      02H             , 03H           ; 227:  MARK(3)
        DB      021H            , 0FFH          ; 229:  parms->Accept
        DB      01H                             ; 231:  Reject

        DB      01aH            , 01H           ; 232:  IdFn->235
        DB      01H                             ; 234:  Reject
        DB      02H             , 03H           ; 235:  MARK(3)
        DB      021H            , 01H           ; 237:  parms->240
        DB      01H                             ; 239:  Reject
        DB      022H            , 01H           ; 240:  tkEQ->243
        DB      00H                             ; 242:  Accept
        DB      02H             , 05H           ; 243:  MARK(5)
        DB      04H             , 0e1H, 08aH    ; 245:  empty->394

        DB      048H            , 01H           ; 248:  tkSEG->251
        DB      01H                             ; 250:  Reject
        DB      022H            , 0e1H, 08aH    ; 251:  tkEQ->394
        DB      00H                             ; 254:  Accept

        DB      011H            , 0FFH          ; 255:  DeflistI2->Accept
        DB      01H                             ; 257:  Reject

        DB      012H            , 0FFH          ; 258:  DeflistI4->Accept
        DB      01H                             ; 260:  Reject

        DB      013H            , 0FFH          ; 261:  DeflistR4->Accept
        DB      01H                             ; 263:  Reject

        DB      014H            , 0FFH          ; 264:  DeflistR8->Accept
        DB      01H                             ; 266:  Reject

        DB      015H            , 0FFH          ; 267:  DeflistSD->Accept
        DB      01H                             ; 269:  Reject

        DB      049H            , 02H           ; 270:  tkSHARED->274
        DB      04H             , 06H           ; 272:  empty->280
        DB      0dH             , 01H           ; 274:  ACTIONidShared->277
        DB      01H                             ; 276:  Reject
        DB      03H                             ; 277:  EMIT(opShared)
        DW      opShared                        
        DB      03H                             ; 280:  EMIT(opStDim)
        DW      opStDim                         
        DB      09H             , 01H           ; 283:  EMITFFFF->286
        DB      01H                             ; 285:  Reject
        DB      017H            , 01H           ; 286:  IdAryDim->289
        DB      01H                             ; 288:  Reject
        DB      026H            , 0dbH          ; 289:  tkComma->286
        DB      00H                             ; 291:  Accept

        DB      04fH            , 0fH           ; 292:  tkWHILE->309
        DB      04eH            , 04H           ; 294:  tkUNTIL->300
        DB      03H                             ; 296:  EMIT(opStDo)
        DW      opStDo                          
        DB      00H                             ; 299:  Accept
        DB      016H            , 01H           ; 300:  Exp->303
        DB      01H                             ; 302:  Reject
        DB      03H                             ; 303:  EMIT(opStDoUntil)
        DW      opStDoUntil                     
        DB      09H             , 0FFH          ; 306:  EMITFFFF->Accept
        DB      01H                             ; 308:  Reject
        DB      016H            , 01H           ; 309:  Exp->312
        DB      01H                             ; 311:  Reject
        DB      03H                             ; 312:  EMIT(opStDoWhile)
        DW      opStDoWhile                     
        DB      09H             , 0FFH          ; 315:  EMITFFFF->Accept
        DB      01H                             ; 317:  Reject

        DB      03H                             ; 318:  EMIT(opEvPen)
        DW      opEvPen                         
        DB      04H             , 014H          ; 321:  empty->343

        DB      016H            , 01H           ; 323:  Exp->326
        DB      01H                             ; 325:  Reject
        DB      03H                             ; 326:  EMIT(opStPlay)
        DW      opStPlay                        
        DB      00H                             ; 329:  Accept

        DB      03H                             ; 330:  EMIT(opEvPlay0)
        DW      opEvPlay0                       
        DB      04H             , 08H           ; 333:  empty->343

        DB      03H                             ; 335:  EMIT(opEvTimer0)
        DW      opEvTimer0                      
        DB      04H             , 03H           ; 338:  empty->343

        DB      03H                             ; 340:  EMIT(opEvUEvent)
        DW      opEvUEvent                      
        DB      07H             , 0FFH          ; 343:  evSwitch->Accept
        DB      01H                             ; 345:  Reject

        DB      016H            , 01H           ; 346:  Exp->349
        DB      01H                             ; 348:  Reject
        DB      03H                             ; 349:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 352:  Accept

        DB      045H            , 0dH           ; 353:  tkON->368
        DB      044H            , 07H           ; 355:  tkOFF->364
        DB      04aH            , 01H           ; 357:  tkSTOP->360
        DB      01H                             ; 359:  Reject
        DB      03H                             ; 360:  EMIT(opEvStop)
        DW      opEvStop                        
        DB      00H                             ; 363:  Accept
        DB      03H                             ; 364:  EMIT(opEvOff)
        DW      opEvOff                         
        DB      00H                             ; 367:  Accept
        DB      03H                             ; 368:  EMIT(opEvOn)
        DW      opEvOn                          
        DB      00H                             ; 371:  Accept

        DB      016H            , 01H           ; 372:  Exp->375
        DB      01H                             ; 374:  Reject

        DB      026H            , 011H          ; 375:  tkComma->394
        DB      01H                             ; 377:  Reject

        DB      03H                             ; 378:  EMIT(UNDEFINED)
        DW      UNDEFINED                       
        DB      00H                             ; 381:  Accept

        DB      024H            , 01H           ; 382:  tkLParen->385
        DB      01H                             ; 384:  Reject
        DB      016H            , 01H           ; 385:  Exp->388
        DB      01H                             ; 387:  Reject
        DB      025H            , 0FFH          ; 388:  tkRParen->Accept
        DB      01H                             ; 390:  Reject

        DB      026H            , 01H           ; 391:  tkComma->394
        DB      00H                             ; 393:  Accept
        DB      016H            , 0FFH          ; 394:  Exp->Accept
        DB      01H                             ; 396:  Reject

; state table = 397 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
