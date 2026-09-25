        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 17:57:09 2026


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
        DW      384     ; caseItem
        DW      387     ; commaExp
        DW      393     ; evSwitch
        DW      412     ; expCommaExp
        DW      421     ; EMITFFFF
        DW      425     ; fn1arg
        DW      434     ; optCommaExp


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
        DB      019H            , 01H           ; 28:  IdCallArg->31
        DB      01H                             ; 30:  Reject
        DB      026H            , 03H           ; 31:  tkComma->36
        DB      025H            , 0FFH          ; 33:  tkRParen->Accept
        DB      01H                             ; 35:  Reject
        DB      019H            , 0d9H          ; 36:  IdCallArg->31
        DB      01H                             ; 38:  Reject

        DB      02H             , 01H           ; 39:  MARK(1)
        DB      01eH            , 01H           ; 41:  IdSubRef->44
        DB      01H                             ; 43:  Reject
        DB      024H            , 01H           ; 44:  tkLParen->47
        DB      00H                             ; 46:  Accept
        DB      016H            , 01H           ; 47:  Exp->50
        DB      01H                             ; 49:  Reject
        DB      026H            , 03H           ; 50:  tkComma->55
        DB      025H            , 0FFH          ; 52:  tkRParen->Accept
        DB      01H                             ; 54:  Reject
        DB      016H            , 0d9H          ; 55:  Exp->50
        DB      01H                             ; 57:  Reject

        DB      042H            , 09H           ; 58:  tkELSE->69
        DB      05H             , 01H           ; 60:  caseItem->63
        DB      01H                             ; 62:  Reject
        DB      026H            , 01H           ; 63:  tkComma->66
        DB      00H                             ; 65:  Accept
        DB      05H             , 0dbH          ; 66:  caseItem->63
        DB      01H                             ; 68:  Reject
        DB      03H                             ; 69:  EMIT(opStCaseElse)
        DW      opStCaseElse                    
        DB      00H                             ; 72:  Accept

        DB      016H            , 01H           ; 73:  Exp->76
        DB      01H                             ; 75:  Reject
        DB      03H                             ; 76:  EMIT(opStChain)
        DW      opStChain                       
        DB      00H                             ; 79:  Accept

        DB      016H            , 01H           ; 80:  Exp->83
        DB      01H                             ; 82:  Reject
        DB      03H                             ; 83:  EMIT(opStChdir)
        DW      opStChdir                       
        DB      00H                             ; 86:  Accept

        DB      010H            , 01H           ; 87:  coordStep->90
        DB      01H                             ; 89:  Reject
        DB      06H             , 01H           ; 90:  commaExp->93
        DB      01H                             ; 92:  Reject
        DB      026H            , 01H           ; 93:  tkComma->96
        DB      00H                             ; 95:  Accept
        DB      016H            , 02H           ; 96:  Exp->100
        DB      04H             , 02H           ; 98:  empty->102
        DB      02H             , 01H           ; 100:  MARK(1)
        DB      0eH             , 01H           ; 102:  CommaNoEos->105
        DB      00H                             ; 104:  Accept
        DB      016H            , 03H           ; 105:  Exp->110
        DB      03H                             ; 107:  EMIT(opNull)
        DW      opNull                          
        DB      03H                             ; 110:  EMIT(opCircleStart)
        DW      opCircleStart                   
        DB      0eH             , 01H           ; 113:  CommaNoEos->116
        DB      00H                             ; 115:  Accept
        DB      016H            , 02H           ; 116:  Exp->120
        DB      04H             , 03H           ; 118:  empty->123
        DB      03H                             ; 120:  EMIT(opCircleEnd)
        DW      opCircleEnd                     
        DB      06H             , 01H           ; 123:  commaExp->126
        DB      00H                             ; 125:  Accept
        DB      03H                             ; 126:  EMIT(opCircleAspect)
        DW      opCircleAspect                  
        DB      00H                             ; 129:  Accept

        DB      01fH            , 0FFH          ; 130:  NArgsMax3->Accept
        DB      01H                             ; 132:  Reject

        DB      020H            , 01H           ; 133:  optFilenum->136
        DB      00H                             ; 135:  Accept
        DB      026H            , 01H           ; 136:  tkComma->139
        DB      00H                             ; 138:  Accept
        DB      020H            , 0dbH          ; 139:  optFilenum->136
        DB      01H                             ; 141:  Reject

        DB      016H            , 03H           ; 142:  Exp->147
        DB      03H                             ; 144:  EMIT(opUndef)
        DW      opUndef                         
        DB      03H                             ; 147:  EMIT(opStCls)
        DW      opStCls                         
        DB      00H                             ; 150:  Accept

        DB      01fH            , 0FFH          ; 151:  NArgsMax3->Accept
        DB      01H                             ; 153:  Reject

        DB      0aH             , 01H           ; 154:  fn1arg->157
        DB      01H                             ; 156:  Reject
        DB      03H                             ; 157:  EMIT(opEvCom)
        DW      opEvCom                         
        DB      07H             , 0FFH          ; 160:  evSwitch->Accept
        DB      01H                             ; 162:  Reject

        DB      049H            , 011H          ; 163:  tkSHARED->182
        DB      03H                             ; 165:  EMIT(opStCommon)
        DW      opStCommon                      
        DB      09H             , 01H           ; 168:  EMITFFFF->171
        DB      01H                             ; 170:  Reject
        DB      023H            , 03H           ; 171:  tkDiv->176
        DB      09H             , 01eH          ; 173:  EMITFFFF->205
        DB      01H                             ; 175:  Reject
        DB      01cH            , 01H           ; 176:  IdNamCom->179
        DB      01H                             ; 178:  Reject
        DB      023H            , 018H          ; 179:  tkDiv->205
        DB      01H                             ; 181:  Reject
        DB      03H                             ; 182:  EMIT(opShared)
        DW      opShared                        
        DB      03H                             ; 185:  EMIT(opStCommon)
        DW      opStCommon                      
        DB      09H             , 01H           ; 188:  EMITFFFF->191
        DB      01H                             ; 190:  Reject
        DB      023H            , 03H           ; 191:  tkDiv->196
        DB      09H             , 07H           ; 193:  EMITFFFF->202
        DB      01H                             ; 195:  Reject
        DB      01cH            , 01H           ; 196:  IdNamCom->199
        DB      01H                             ; 198:  Reject
        DB      023H            , 01H           ; 199:  tkDiv->202
        DB      01H                             ; 201:  Reject
        DB      0dH             , 01H           ; 202:  ACTIONidShared->205
        DB      01H                             ; 204:  Reject
        DB      0cH             , 01H           ; 205:  ACTIONidCommon->208
        DB      01H                             ; 207:  Reject
        DB      018H            , 01H           ; 208:  IdAryI->211
        DB      01H                             ; 210:  Reject
        DB      026H            , 01H           ; 211:  tkComma->214
        DB      00H                             ; 213:  Accept
        DB      018H            , 0dbH          ; 214:  IdAryI->211
        DB      01H                             ; 216:  Reject

        DB      03H                             ; 217:  EMIT(opStConst)
        DW      opStConst                       
        DB      0fH             , 01H           ; 220:  ConstAssign->223
        DB      01H                             ; 222:  Reject
        DB      026H            , 01H           ; 223:  tkComma->226
        DB      00H                             ; 225:  Accept
        DB      0fH             , 0dbH          ; 226:  ConstAssign->223
        DB      01H                             ; 228:  Reject

        DB      022H            , 01H           ; 229:  tkEQ->232
        DB      01H                             ; 231:  Reject
        DB      016H            , 01H           ; 232:  Exp->235
        DB      01H                             ; 234:  Reject
        DB      03H                             ; 235:  EMIT(opStDate_)
        DW      opStDate_                       
        DB      00H                             ; 238:  Accept

        DB      043H            , 06H           ; 239:  tkFUNCTION->247
        DB      04bH            , 01H           ; 241:  tkSUB->244
        DB      01H                             ; 243:  Reject
        DB      01dH            , 04H           ; 244:  IdSubDecl->250
        DB      01H                             ; 246:  Reject
        DB      01bH            , 01H           ; 247:  IdFuncDecl->250
        DB      01H                             ; 249:  Reject
        DB      02H             , 03H           ; 250:  MARK(3)
        DB      021H            , 0FFH          ; 252:  parms->Accept
        DB      01H                             ; 254:  Reject

        DB      01aH            , 01H           ; 255:  IdFn->258
        DB      01H                             ; 257:  Reject
        DB      02H             , 03H           ; 258:  MARK(3)
        DB      021H            , 01H           ; 260:  parms->263
        DB      01H                             ; 262:  Reject
        DB      022H            , 01H           ; 263:  tkEQ->266
        DB      00H                             ; 265:  Accept
        DB      02H             , 05H           ; 266:  MARK(5)
        DB      016H            , 0FFH          ; 268:  Exp->Accept
        DB      01H                             ; 270:  Reject

        DB      048H            , 01H           ; 271:  tkSEG->274
        DB      01H                             ; 273:  Reject
        DB      022H            , 01H           ; 274:  tkEQ->277
        DB      00H                             ; 276:  Accept
        DB      016H            , 0FFH          ; 277:  Exp->Accept
        DB      01H                             ; 279:  Reject

        DB      011H            , 0FFH          ; 280:  DeflistI2->Accept
        DB      01H                             ; 282:  Reject

        DB      012H            , 0FFH          ; 283:  DeflistI4->Accept
        DB      01H                             ; 285:  Reject

        DB      013H            , 0FFH          ; 286:  DeflistR4->Accept
        DB      01H                             ; 288:  Reject

        DB      014H            , 0FFH          ; 289:  DeflistR8->Accept
        DB      01H                             ; 291:  Reject

        DB      015H            , 0FFH          ; 292:  DeflistSD->Accept
        DB      01H                             ; 294:  Reject

        DB      049H            , 02H           ; 295:  tkSHARED->299
        DB      04H             , 06H           ; 297:  empty->305
        DB      0dH             , 01H           ; 299:  ACTIONidShared->302
        DB      01H                             ; 301:  Reject
        DB      03H                             ; 302:  EMIT(opShared)
        DW      opShared                        
        DB      03H                             ; 305:  EMIT(opStDim)
        DW      opStDim                         
        DB      09H             , 01H           ; 308:  EMITFFFF->311
        DB      01H                             ; 310:  Reject
        DB      017H            , 01H           ; 311:  IdAryDim->314
        DB      01H                             ; 313:  Reject
        DB      026H            , 01H           ; 314:  tkComma->317
        DB      00H                             ; 316:  Accept
        DB      017H            , 0dbH          ; 317:  IdAryDim->314
        DB      01H                             ; 319:  Reject

        DB      04fH            , 0fH           ; 320:  tkWHILE->337
        DB      04eH            , 04H           ; 322:  tkUNTIL->328
        DB      03H                             ; 324:  EMIT(opStDo)
        DW      opStDo                          
        DB      00H                             ; 327:  Accept
        DB      016H            , 01H           ; 328:  Exp->331
        DB      01H                             ; 330:  Reject
        DB      03H                             ; 331:  EMIT(opStDoUntil)
        DW      opStDoUntil                     
        DB      09H             , 0FFH          ; 334:  EMITFFFF->Accept
        DB      01H                             ; 336:  Reject
        DB      016H            , 01H           ; 337:  Exp->340
        DB      01H                             ; 339:  Reject
        DB      03H                             ; 340:  EMIT(opStDoWhile)
        DW      opStDoWhile                     
        DB      09H             , 0FFH          ; 343:  EMITFFFF->Accept
        DB      01H                             ; 345:  Reject

        DB      03H                             ; 346:  EMIT(opEvPen)
        DW      opEvPen                         
        DB      07H             , 0FFH          ; 349:  evSwitch->Accept
        DB      01H                             ; 351:  Reject

        DB      016H            , 01H           ; 352:  Exp->355
        DB      01H                             ; 354:  Reject
        DB      03H                             ; 355:  EMIT(opStPlay)
        DW      opStPlay                        
        DB      00H                             ; 358:  Accept

        DB      03H                             ; 359:  EMIT(opEvPlay0)
        DW      opEvPlay0                       
        DB      07H             , 0FFH          ; 362:  evSwitch->Accept
        DB      01H                             ; 364:  Reject

        DB      03H                             ; 365:  EMIT(opEvTimer0)
        DW      opEvTimer0                      
        DB      07H             , 0FFH          ; 368:  evSwitch->Accept
        DB      01H                             ; 370:  Reject

        DB      03H                             ; 371:  EMIT(opEvUEvent)
        DW      opEvUEvent                      
        DB      07H             , 0FFH          ; 374:  evSwitch->Accept
        DB      01H                             ; 376:  Reject

        DB      016H            , 01H           ; 377:  Exp->380
        DB      01H                             ; 379:  Reject
        DB      03H                             ; 380:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 383:  Accept

        DB      016H            , 0FFH          ; 384:  Exp->Accept
        DB      01H                             ; 386:  Reject

        DB      026H            , 01H           ; 387:  tkComma->390
        DB      01H                             ; 389:  Reject
        DB      016H            , 0FFH          ; 390:  Exp->Accept
        DB      01H                             ; 392:  Reject

        DB      045H            , 0dH           ; 393:  tkON->408
        DB      044H            , 07H           ; 395:  tkOFF->404
        DB      04aH            , 01H           ; 397:  tkSTOP->400
        DB      01H                             ; 399:  Reject
        DB      03H                             ; 400:  EMIT(opEvStop)
        DW      opEvStop                        
        DB      00H                             ; 403:  Accept
        DB      03H                             ; 404:  EMIT(opEvOff)
        DW      opEvOff                         
        DB      00H                             ; 407:  Accept
        DB      03H                             ; 408:  EMIT(opEvOn)
        DW      opEvOn                          
        DB      00H                             ; 411:  Accept

        DB      016H            , 01H           ; 412:  Exp->415
        DB      01H                             ; 414:  Reject
        DB      026H            , 01H           ; 415:  tkComma->418
        DB      01H                             ; 417:  Reject
        DB      016H            , 0FFH          ; 418:  Exp->Accept
        DB      01H                             ; 420:  Reject

        DB      03H                             ; 421:  EMIT(UNDEFINED)
        DW      UNDEFINED                       
        DB      00H                             ; 424:  Accept

        DB      024H            , 01H           ; 425:  tkLParen->428
        DB      01H                             ; 427:  Reject
        DB      016H            , 01H           ; 428:  Exp->431
        DB      01H                             ; 430:  Reject
        DB      025H            , 0FFH          ; 431:  tkRParen->Accept
        DB      01H                             ; 433:  Reject

        DB      026H            , 01H           ; 434:  tkComma->437
        DB      00H                             ; 436:  Accept
        DB      016H            , 0FFH          ; 437:  Exp->Accept
        DB      01H                             ; 439:  Reject

; state table = 440 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
