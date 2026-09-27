        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Fri Jun 12 21:11:31 2026


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
        EXTRN   NtACTIONidStatic:NEAR
        EXTRN   NtAssignment:NEAR
        EXTRN   NtCaseRelation:NEAR
        EXTRN   NtCommaNoEos:NEAR
        EXTRN   NtConstAssign:NEAR
        EXTRN   NtDeflistI2:NEAR
        EXTRN   NtDeflistI4:NEAR
        EXTRN   NtDeflistR4:NEAR
        EXTRN   NtDeflistR8:NEAR
        EXTRN   NtDeflistSD:NEAR
        EXTRN   NtEndPrint:NEAR
        EXTRN   NtEndPrintExp:NEAR
        EXTRN   NtErrIfNot1st:NEAR
        EXTRN   NtExp:NEAR
        EXTRN   NtIdAry:NEAR
        EXTRN   NtIdAryDim:NEAR
        EXTRN   NtIdAryRedim:NEAR
        EXTRN   NtIdAryElem:NEAR
        EXTRN   NtIdAryElemRef:NEAR
        EXTRN   NtIdAryGetPut:NEAR
        EXTRN   NtIdAryI:NEAR
        EXTRN   NtIdArray:NEAR
        EXTRN   NtIdCallArg:NEAR
        EXTRN   NtIdFor:NEAR
        EXTRN   NtIdFn:NEAR
        EXTRN   NtIdFuncDecl:NEAR
        EXTRN   NtIdFuncDef:NEAR
        EXTRN   NtIdType:NEAR
        EXTRN   NtIdNamCom:NEAR
        EXTRN   NtIdParm:NEAR
        EXTRN   NtIdSubDecl:NEAR
        EXTRN   NtIdSubDef:NEAR
        EXTRN   NtIdSubRef:NEAR
        EXTRN   NtIfStmt:NEAR
        EXTRN   NtLabLn:NEAR
        EXTRN   NtLitString:NEAR
        EXTRN   NtLit0:NEAR
        EXTRN   NtLit1:NEAR
        EXTRN   NtLn:NEAR
        EXTRN   NtNArgsMax3:NEAR
        EXTRN   NtNArgsMax4:NEAR
        EXTRN   NtNArgsMax5:NEAR
        EXTRN   NtRwB:NEAR
        EXTRN   NtRwF:NEAR
        EXTRN   NtRwBF:NEAR
        EXTRN   NtStatement:NEAR
        EXTRN   NtStatementList:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD
        DW      2463    ; AsClausePrim
        DW      2511    ; AsClause
        DW      2518    ; AsClauseAny
        DW      2535    ; caseItem
        DW      2722    ; commaExp
        DW      2556    ; commaOptExp
        DW      2562    ; commaOptExpNil
        DW      2571    ; commaOptExpNull
        DW      2580    ; coordStep
        DW      2597    ; coord2Step
        DW      2614    ; EMITFFFF
        DW      2618    ; event
        DW      2691    ; evSwitch
        DW      2713    ; exp12
        DW      2719    ; expCommaExp
        DW      2725    ; fn1arg
        DW      2731    ; fn12arg
        DW      2737    ; fn2arg
        DW      2743    ; fn23arg
        DW      2752    ; fnBoundArg
        DW      2761    ; lbsExpComma
        DW      2775    ; lbsInpExpComma
        DW      2790    ; optCommaExp
        DW      2793    ; optFilenum
        DW      2805    ; parms
        DW      2815    ; parms1
        DW      2831    ; printItem
        DW      2882    ; printList
        DW      2900    ; printUsingItem


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtACTIONidCommon
        DW      NtACTIONidShared
        DW      NtACTIONidStatic
        DW      NtAssignment
        DW      NtCaseRelation
        DW      NtCommaNoEos
        DW      NtConstAssign
        DW      NtDeflistI2
        DW      NtDeflistI4
        DW      NtDeflistR4
        DW      NtDeflistR8
        DW      NtDeflistSD
        DW      NtEndPrint
        DW      NtEndPrintExp
        DW      NtErrIfNot1st
        DW      NtExp
        DW      NtIdAry
        DW      NtIdAryDim
        DW      NtIdAryRedim
        DW      NtIdAryElem
        DW      NtIdAryElemRef
        DW      NtIdAryGetPut
        DW      NtIdAryI
        DW      NtIdArray
        DW      NtIdCallArg
        DW      NtIdFor
        DW      NtIdFn
        DW      NtIdFuncDecl
        DW      NtIdFuncDef
        DW      NtIdType
        DW      NtIdNamCom
        DW      NtIdParm
        DW      NtIdSubDecl
        DW      NtIdSubDef
        DW      NtIdSubRef
        DW      NtIfStmt
        DW      NtLabLn
        DW      NtLitString
        DW      NtLit0
        DW      NtLit1
        DW      NtLn
        DW      NtNArgsMax3
        DW      NtNArgsMax4
        DW      NtNArgsMax5
        DW      NtRwB
        DW      NtRwF
        DW      NtRwBF
        DW      NtStatement
        DW      NtStatementList

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      0       ; ACTIONidCommon
        DW      0       ; ACTIONidShared
        DW      0       ; ACTIONidStatic
        DW      MSG_ExpAssignment
        DW      MSG_ExpRel
        DW      0       ; CommaNoEos
        DW      0       ; ConstAssign
        DW      0       ; DeflistI2
        DW      0       ; DeflistI4
        DW      0       ; DeflistR4
        DW      0       ; DeflistR8
        DW      0       ; DeflistSD
        DW      0       ; EndPrint
        DW      0       ; EndPrintExp
        DW      0       ; ErrIfNot1st
        DW      MSG_ExpExp
        DW      MSG_ExpVar
        DW      MSG_ExpVar
        DW      MSG_ExpVar
        DW      MSG_ExpVar
        DW      MSG_ExpVar
        DW      MSG_ExpVar
        DW      MSG_ExpVar
        DW      MSG_ExpVar
        DW      MSG_ExpIdCallArg
        DW      MSG_ExpVar
        DW      0       ; IdFn
        DW      0       ; IdFuncDecl
        DW      0       ; IdFuncDef
        DW      MSG_ExpId
        DW      0       ; IdNamCom
        DW      MSG_ExpIdParm
        DW      0       ; IdSubDecl
        DW      0       ; IdSubDef
        DW      0       ; IdSubRef
        DW      0       ; IfStmt
        DW      MSG_ExpLabLn
        DW      MSG_ExpLitString
        DW      MSG_ExpLit0
        DW      MSG_ExpLit1
        DW      MSG_ExpLabLn
        DW      MSG_ExpNArgs
        DW      MSG_ExpNArgs
        DW      MSG_ExpNArgs
        DW      MSG_ExpRwB
        DW      MSG_ExpRwF
        DW      MSG_ExpRwBF
        DW      MSG_ExpStatement
        DW      MSG_ExpStatement

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      03H                             ; 0:  EMIT(opStBeep)
        DW      opStBeep                        
        DB      00H                             ; 3:  Accept

        DB      031H            , 0eaH, 09cH    ; 4:  Exp->2716
        DB      01H                             ; 7:  Reject

        DB      013H            , 01H           ; 8:  expCommaExp->11
        DB      01H                             ; 10:  Reject
        DB      09H             , 01H           ; 11:  commaExp->14
        DB      01H                             ; 13:  Reject
        DB      03H                             ; 14:  EMIT(opStBsave)
        DW      opStBsave                       
        DB      00H                             ; 17:  Accept

        DB      02H             , 01H           ; 18:  MARK(1)
        DB      044H            , 01H           ; 20:  IdSubRef->23
        DB      01H                             ; 22:  Reject
        DB      05fH            , 01H           ; 23:  tkLParen->26
        DB      00H                             ; 25:  Accept
        DB      03aH            , 01H           ; 26:  IdCallArg->29
        DB      01H                             ; 28:  Reject
        DB      063H            , 03H           ; 29:  tkComma->34
        DB      060H            , 0FFH          ; 31:  tkRParen->Accept
        DB      01H                             ; 33:  Reject
        DB      03aH            , 0d9H          ; 34:  IdCallArg->29
        DB      01H                             ; 36:  Reject

        DB      02H             , 01H           ; 37:  MARK(1)
        DB      044H            , 01H           ; 39:  IdSubRef->42
        DB      01H                             ; 41:  Reject
        DB      05fH            , 01H           ; 42:  tkLParen->45
        DB      00H                             ; 44:  Accept
        DB      031H            , 01H           ; 45:  Exp->48
        DB      01H                             ; 47:  Reject
        DB      063H            , 03H           ; 48:  tkComma->53
        DB      060H            , 0FFH          ; 50:  tkRParen->Accept
        DB      01H                             ; 52:  Reject
        DB      031H            , 0d9H          ; 53:  Exp->48
        DB      01H                             ; 55:  Reject

        DB      0a4H            , 09H           ; 56:  tkELSE->67
        DB      08H             , 01H           ; 58:  caseItem->61
        DB      01H                             ; 60:  Reject
        DB      063H            , 01H           ; 61:  tkComma->64
        DB      00H                             ; 63:  Accept
        DB      08H             , 0dbH          ; 64:  caseItem->61
        DB      01H                             ; 66:  Reject
        DB      03H                             ; 67:  EMIT(opStCaseElse)
        DW      opStCaseElse                    
        DB      00H                             ; 70:  Accept

        DB      031H            , 01H           ; 71:  Exp->74
        DB      01H                             ; 73:  Reject
        DB      03H                             ; 74:  EMIT(opStChain)
        DW      opStChain                       
        DB      00H                             ; 77:  Accept

        DB      031H            , 01H           ; 78:  Exp->81
        DB      01H                             ; 80:  Reject
        DB      03H                             ; 81:  EMIT(opStChdir)
        DW      opStChdir                       
        DB      00H                             ; 84:  Accept

        DB      0dH             , 01H           ; 85:  coordStep->88
        DB      01H                             ; 87:  Reject
        DB      09H             , 01H           ; 88:  commaExp->91
        DB      01H                             ; 90:  Reject
        DB      063H            , 01H           ; 91:  tkComma->94
        DB      00H                             ; 93:  Accept
        DB      031H            , 02H           ; 94:  Exp->98
        DB      04H             , 02H           ; 96:  empty->100
        DB      02H             , 01H           ; 98:  MARK(1)
        DB      027H            , 01H           ; 100:  CommaNoEos->103
        DB      00H                             ; 102:  Accept
        DB      031H            , 03H           ; 103:  Exp->108
        DB      03H                             ; 105:  EMIT(opNull)
        DW      opNull                          
        DB      03H                             ; 108:  EMIT(opCircleStart)
        DW      opCircleStart                   
        DB      027H            , 01H           ; 111:  CommaNoEos->114
        DB      00H                             ; 113:  Accept
        DB      031H            , 02H           ; 114:  Exp->118
        DB      04H             , 03H           ; 116:  empty->121
        DB      03H                             ; 118:  EMIT(opCircleEnd)
        DW      opCircleEnd                     
        DB      09H             , 01H           ; 121:  commaExp->124
        DB      00H                             ; 123:  Accept
        DB      03H                             ; 124:  EMIT(opCircleAspect)
        DW      opCircleAspect                  
        DB      00H                             ; 127:  Accept

        DB      04bH            , 0FFH          ; 128:  NArgsMax3->Accept
        DB      01H                             ; 130:  Reject

        DB      01cH            , 01H           ; 131:  optFilenum->134
        DB      00H                             ; 133:  Accept
        DB      063H            , 01H           ; 134:  tkComma->137
        DB      00H                             ; 136:  Accept
        DB      01cH            , 0dbH          ; 137:  optFilenum->134
        DB      01H                             ; 139:  Reject

        DB      031H            , 03H           ; 140:  Exp->145
        DB      03H                             ; 142:  EMIT(opUndef)
        DW      opUndef                         
        DB      03H                             ; 145:  EMIT(opStCls)
        DW      opStCls                         
        DB      00H                             ; 148:  Accept

        DB      014H            , 01H           ; 149:  fn1arg->152
        DB      01H                             ; 151:  Reject
        DB      03H                             ; 152:  EMIT(opEvCom)
        DW      opEvCom                         
        DB      04H             , 0e6H, 0afH    ; 155:  empty->1711

        DB      0e0H, 039H      , 011H          ; 158:  tkSHARED->178
        DB      03H                             ; 161:  EMIT(opStCommon)
        DW      opStCommon                      
        DB      0fH             , 01H           ; 164:  EMITFFFF->167
        DB      01H                             ; 166:  Reject
        DB      065H            , 03H           ; 167:  tkDiv->172
        DB      0fH             , 01eH          ; 169:  EMITFFFF->201
        DB      01H                             ; 171:  Reject
        DB      040H            , 01H           ; 172:  IdNamCom->175
        DB      01H                             ; 174:  Reject
        DB      065H            , 018H          ; 175:  tkDiv->201
        DB      01H                             ; 177:  Reject
        DB      03H                             ; 178:  EMIT(opShared)
        DW      opShared                        
        DB      03H                             ; 181:  EMIT(opStCommon)
        DW      opStCommon                      
        DB      0fH             , 01H           ; 184:  EMITFFFF->187
        DB      01H                             ; 186:  Reject
        DB      065H            , 03H           ; 187:  tkDiv->192
        DB      0fH             , 07H           ; 189:  EMITFFFF->198
        DB      01H                             ; 191:  Reject
        DB      040H            , 01H           ; 192:  IdNamCom->195
        DB      01H                             ; 194:  Reject
        DB      065H            , 01H           ; 195:  tkDiv->198
        DB      01H                             ; 197:  Reject
        DB      023H            , 01H           ; 198:  ACTIONidShared->201
        DB      01H                             ; 200:  Reject
        DB      022H            , 01H           ; 201:  ACTIONidCommon->204
        DB      01H                             ; 203:  Reject
        DB      038H            , 01H           ; 204:  IdAryI->207
        DB      01H                             ; 206:  Reject
        DB      063H            , 01H           ; 207:  tkComma->210
        DB      00H                             ; 209:  Accept
        DB      038H            , 0dbH          ; 210:  IdAryI->207
        DB      01H                             ; 212:  Reject

        DB      03H                             ; 213:  EMIT(opStConst)
        DW      opStConst                       
        DB      028H            , 01H           ; 216:  ConstAssign->219
        DB      01H                             ; 218:  Reject
        DB      063H            , 01H           ; 219:  tkComma->222
        DB      00H                             ; 221:  Accept
        DB      028H            , 0dbH          ; 222:  ConstAssign->219
        DB      01H                             ; 224:  Reject

        DB      069H            , 01H           ; 225:  tkEQ->228
        DB      01H                             ; 227:  Reject
        DB      031H            , 01H           ; 228:  Exp->231
        DB      01H                             ; 230:  Reject
        DB      03H                             ; 231:  EMIT(opStDate_)
        DW      opStDate_                       
        DB      00H                             ; 234:  Accept

        DB      0bbH            , 07H           ; 235:  tkFUNCTION->244
        DB      0e0H, 04bH      , 01H           ; 237:  tkSUB->241
        DB      01H                             ; 240:  Reject
        DB      042H            , 04H           ; 241:  IdSubDecl->247
        DB      01H                             ; 243:  Reject
        DB      03dH            , 01H           ; 244:  IdFuncDecl->247
        DB      01H                             ; 246:  Reject
        DB      02H             , 03H           ; 247:  MARK(3)
        DB      01dH            , 0FFH          ; 249:  parms->Accept
        DB      01H                             ; 251:  Reject

        DB      03cH            , 01H           ; 252:  IdFn->255
        DB      01H                             ; 254:  Reject
        DB      02H             , 03H           ; 255:  MARK(3)
        DB      01dH            , 01H           ; 257:  parms->260
        DB      01H                             ; 259:  Reject
        DB      069H            , 01H           ; 260:  tkEQ->263
        DB      00H                             ; 262:  Accept
        DB      02H             , 05H           ; 263:  MARK(5)
        DB      04H             , 0eaH, 0ebH    ; 265:  empty->2795

        DB      0e0H, 035H      , 01H           ; 268:  tkSEG->272
        DB      01H                             ; 271:  Reject
        DB      069H            , 0eaH, 0ebH    ; 272:  tkEQ->2795
        DB      00H                             ; 275:  Accept

        DB      029H            , 0FFH          ; 276:  DeflistI2->Accept
        DB      01H                             ; 278:  Reject

        DB      02aH            , 0FFH          ; 279:  DeflistI4->Accept
        DB      01H                             ; 281:  Reject

        DB      02bH            , 0FFH          ; 282:  DeflistR4->Accept
        DB      01H                             ; 284:  Reject

        DB      02cH            , 0FFH          ; 285:  DeflistR8->Accept
        DB      01H                             ; 287:  Reject

        DB      02dH            , 0FFH          ; 288:  DeflistSD->Accept
        DB      01H                             ; 290:  Reject

        DB      0e0H, 039H      , 02H           ; 291:  tkSHARED->296
        DB      04H             , 06H           ; 294:  empty->302
        DB      023H            , 01H           ; 296:  ACTIONidShared->299
        DB      01H                             ; 298:  Reject
        DB      03H                             ; 299:  EMIT(opShared)
        DW      opShared                        
        DB      03H                             ; 302:  EMIT(opStDim)
        DW      opStDim                         
        DB      0fH             , 01H           ; 305:  EMITFFFF->308
        DB      01H                             ; 307:  Reject
        DB      033H            , 01H           ; 308:  IdAryDim->311
        DB      01H                             ; 310:  Reject
        DB      063H            , 01H           ; 311:  tkComma->314
        DB      00H                             ; 313:  Accept
        DB      033H            , 0dbH          ; 314:  IdAryDim->311
        DB      01H                             ; 316:  Reject

        DB      0e0H, 064H      , 010H          ; 317:  tkWHILE->336
        DB      0e0H, 05bH      , 04H           ; 320:  tkUNTIL->327
        DB      03H                             ; 323:  EMIT(opStDo)
        DW      opStDo                          
        DB      00H                             ; 326:  Accept
        DB      031H            , 01H           ; 327:  Exp->330
        DB      01H                             ; 329:  Reject
        DB      03H                             ; 330:  EMIT(opStDoUntil)
        DW      opStDoUntil                     
        DB      0fH             , 0FFH          ; 333:  EMITFFFF->Accept
        DB      01H                             ; 335:  Reject
        DB      031H            , 01H           ; 336:  Exp->339
        DB      01H                             ; 338:  Reject
        DB      03H                             ; 339:  EMIT(opStDoWhile)
        DW      opStDoWhile                     
        DB      04H             , 0e9H, 08fH    ; 342:  empty->2447

        DB      031H            , 01H           ; 345:  Exp->348
        DB      01H                             ; 347:  Reject
        DB      03H                             ; 348:  EMIT(opStDraw)
        DW      opStDraw                        
        DB      00H                             ; 351:  Accept

        DB      031H            , 01H           ; 352:  Exp->355
        DB      01H                             ; 354:  Reject
        DB      0e0H, 050H      , 01H           ; 355:  tkTHEN->359
        DB      01H                             ; 358:  Reject
        DB      03H                             ; 359:  EMIT(opStElseIf)
        DW      opStElseIf                      
        DB      0fH             , 01H           ; 362:  EMITFFFF->365
        DB      01H                             ; 364:  Reject
        DB      052H            , 0FFH          ; 365:  StatementList->Accept
        DB      00H                             ; 367:  Accept

        DB      03H                             ; 368:  EMIT(opStElse)
        DW      opStElse                        
        DB      0fH             , 01H           ; 371:  EMITFFFF->374
        DB      01H                             ; 373:  Reject
        DB      052H            , 0FFH          ; 374:  StatementList->Accept
        DB      00H                             ; 376:  Accept

        DB      09aH            , 028H          ; 377:  tkDEF->419
        DB      0bbH            , 022H          ; 379:  tkFUNCTION->415
        DB      0c0H            , 01bH          ; 381:  tkIF->410
        DB      0e0H, 036H      , 014H          ; 383:  tkSELECT->406
        DB      0e0H, 04bH      , 0dH           ; 386:  tkSUB->402
        DB      0e0H, 056H      , 04H           ; 389:  tkTYPE->396
        DB      03H                             ; 392:  EMIT(opStEnd)
        DW      opStEnd                         
        DB      00H                             ; 395:  Accept
        DB      03H                             ; 396:  EMIT(opStEndType)
        DW      opStEndType                     
        DB      04H             , 0e9H, 08fH    ; 399:  empty->2447
        DB      03H                             ; 402:  EMIT(opStEndProc)
        DW      opStEndProc                     
        DB      00H                             ; 405:  Accept
        DB      03H                             ; 406:  EMIT(opStEndSelect)
        DW      opStEndSelect                   
        DB      00H                             ; 409:  Accept
        DB      03H                             ; 410:  EMIT(opStEndIfBlock)
        DW      opStEndIfBlock                  
        DB      04H             , 010H          ; 413:  empty->431
        DB      03H                             ; 415:  EMIT(opStEndProc)
        DW      opStEndProc                     
        DB      00H                             ; 418:  Accept
        DB      03H                             ; 419:  EMIT(opStEndDef)
        DW      opStEndDef                      
        DB      03H                             ; 422:  EMIT(2)
        DW      2
        DB      04H             , 0e9H, 08fH    ; 425:  empty->2447

        DB      03H                             ; 428:  EMIT(opStEndIfBlock)
        DW      opStEndIfBlock                  
        DB      030H            , 0FFH          ; 431:  ErrIfNot1st->Accept
        DB      01H                             ; 433:  Reject

        DB      031H            , 01H           ; 434:  Exp->437
        DB      01H                             ; 436:  Reject
        DB      03H                             ; 437:  EMIT(opStEnviron)
        DW      opStEnviron                     
        DB      00H                             ; 440:  Accept

        DB      039H            , 01H           ; 441:  IdArray->444
        DB      01H                             ; 443:  Reject
        DB      063H            , 01H           ; 444:  tkComma->447
        DB      00H                             ; 446:  Accept
        DB      039H            , 0dbH          ; 447:  IdArray->444
        DB      01H                             ; 449:  Reject

        DB      031H            , 01H           ; 450:  Exp->453
        DB      01H                             ; 452:  Reject
        DB      03H                             ; 453:  EMIT(opStError)
        DW      opStError                       
        DB      00H                             ; 456:  Accept

        DB      09aH            , 016H          ; 457:  tkDEF->481
        DB      0bbH            , 014H          ; 459:  tkFUNCTION->481
        DB      0e0H, 04bH      , 011H          ; 461:  tkSUB->481
        DB      0a1H            , 09H           ; 464:  tkDO->475
        DB      0b8H            , 01H           ; 466:  tkFOR->469
        DB      01H                             ; 468:  Reject
        DB      03H                             ; 469:  EMIT(opStExitFor)
        DW      opStExitFor                     
        DB      04H             , 0e9H, 08fH    ; 472:  empty->2447
        DB      03H                             ; 475:  EMIT(opStExitDo)
        DW      opStExitDo                      
        DB      04H             , 0e9H, 08fH    ; 478:  empty->2447
        DB      03H                             ; 481:  EMIT(opStExitProc)
        DW      opStExitProc                    
        DB      04H             , 0e9H, 08fH    ; 484:  empty->2447

        DB      01cH            , 01H           ; 487:  optFilenum->490
        DB      01H                             ; 489:  Reject
        DB      03H                             ; 490:  EMIT(opFieldInit)
        DW      opFieldInit                     
        DB      09H             , 01H           ; 493:  commaExp->496
        DB      01H                             ; 495:  Reject
        DB      072H            , 01H           ; 496:  tkAS->499
        DB      01H                             ; 498:  Reject
        DB      035H            , 01H           ; 499:  IdAryElem->502
        DB      01H                             ; 501:  Reject
        DB      03H                             ; 502:  EMIT(opFieldItem)
        DW      opFieldItem                     
        DB      09H             , 01H           ; 505:  commaExp->508
        DB      00H                             ; 507:  Accept
        DB      072H            , 01H           ; 508:  tkAS->511
        DB      01H                             ; 510:  Reject
        DB      035H            , 01H           ; 511:  IdAryElem->514
        DB      01H                             ; 513:  Reject
        DB      03H                             ; 514:  EMIT(opFieldItem)
        DW      opFieldItem                     
        DB      04H             , 0d2H          ; 517:  empty->505

        DB      031H            , 0FFH          ; 519:  Exp->Accept
        DB      00H                             ; 521:  Accept

        DB      03bH            , 01H           ; 522:  IdFor->525
        DB      01H                             ; 524:  Reject
        DB      069H            , 01H           ; 525:  tkEQ->528
        DB      01H                             ; 527:  Reject
        DB      031H            , 01H           ; 528:  Exp->531
        DB      01H                             ; 530:  Reject
        DB      0e0H, 053H      , 01H           ; 531:  tkTO->535
        DB      01H                             ; 534:  Reject
        DB      031H            , 01H           ; 535:  Exp->538
        DB      01H                             ; 537:  Reject
        DB      0e0H, 044H      , 06H           ; 538:  tkSTEP->547
        DB      03H                             ; 541:  EMIT(opStFor)
        DW      opStFor                         
        DB      04H             , 0e3H, 0d7H    ; 544:  empty->983
        DB      031H            , 01H           ; 547:  Exp->550
        DB      01H                             ; 549:  Reject
        DB      03H                             ; 550:  EMIT(opStForStep)
        DW      opStForStep                     
        DB      04H             , 0e3H, 0d7H    ; 553:  empty->983

        DB      030H            , 01H           ; 556:  ErrIfNot1st->559
        DB      01H                             ; 558:  Reject
        DB      03eH            , 01H           ; 559:  IdFuncDef->562
        DB      01H                             ; 561:  Reject
        DB      02H             , 03H           ; 562:  MARK(3)
        DB      01dH            , 01H           ; 564:  parms->567
        DB      01H                             ; 566:  Reject
        DB      0e0H, 043H      , 01H           ; 567:  tkSTATIC->571
        DB      00H                             ; 570:  Accept
        DB      02H             , 04H           ; 571:  MARK(4)
        DB      00H                             ; 573:  Accept

        DB      01cH            , 01H           ; 574:  optFilenum->577
        DB      01H                             ; 576:  Reject
        DB      063H            , 04H           ; 577:  tkComma->583
        DB      03H                             ; 579:  EMIT(opStGet1)
        DW      opStGet1                        
        DB      00H                             ; 582:  Accept
        DB      031H            , 0cH           ; 583:  Exp->597
        DB      063H            , 01H           ; 585:  tkComma->588
        DB      01H                             ; 587:  Reject
        DB      036H            , 01H           ; 588:  IdAryElemRef->591
        DB      01H                             ; 590:  Reject
        DB      03H                             ; 591:  EMIT(opStGetRec2)
        DW      opStGetRec2                     
        DB      04H             , 0e9H, 08fH    ; 594:  empty->2447
        DB      063H            , 04H           ; 597:  tkComma->603
        DB      03H                             ; 599:  EMIT(opStGet2)
        DW      opStGet2                        
        DB      00H                             ; 602:  Accept
        DB      036H            , 01H           ; 603:  IdAryElemRef->606
        DB      01H                             ; 605:  Reject
        DB      03H                             ; 606:  EMIT(opStGetRec3)
        DW      opStGetRec3                     
        DB      04H             , 0e9H, 08fH    ; 609:  empty->2447

        DB      0dH             , 01H           ; 612:  coordStep->615
        DB      01H                             ; 614:  Reject
        DB      064H            , 01H           ; 615:  tkMinus->618
        DB      01H                             ; 617:  Reject
        DB      0eH             , 01H           ; 618:  coord2Step->621
        DB      01H                             ; 620:  Reject
        DB      063H            , 01H           ; 621:  tkComma->624
        DB      01H                             ; 623:  Reject
        DB      037H            , 01H           ; 624:  IdAryGetPut->627
        DB      01H                             ; 626:  Reject
        DB      03H                             ; 627:  EMIT(opStGraphicsGet)
        DW      opStGraphicsGet                 
        DB      00H                             ; 630:  Accept

        DB      03H                             ; 631:  EMIT(opStGosub)
        DW      opStGosub                       
        DB      04H             , 0e4H, 026H    ; 634:  empty->1062

        DB      03H                             ; 637:  EMIT(opStGoto)
        DW      opStGoto                        
        DB      04H             , 0e4H, 026H    ; 640:  empty->1062

        DB      031H            , 01H           ; 643:  Exp->646
        DB      01H                             ; 645:  Reject
        DB      045H            , 0FFH          ; 646:  IfStmt->Accept
        DB      01H                             ; 648:  Reject

        DB      01aH            , 022H          ; 649:  lbsInpExpComma->685
        DB      067H            , 0fH           ; 651:  tkSColon->668
        DB      047H            , 02H           ; 653:  LitString->657
        DB      04H             , 01eH          ; 655:  empty->687
        DB      02H             , 04H           ; 657:  MARK(4)
        DB      067H            , 01aH          ; 659:  tkSColon->687
        DB      063H            , 01H           ; 661:  tkComma->664
        DB      01H                             ; 663:  Reject
        DB      02H             , 01H           ; 664:  MARK(1)
        DB      04H             , 013H          ; 666:  empty->687
        DB      02H             , 02H           ; 668:  MARK(2)
        DB      047H            , 02H           ; 670:  LitString->674
        DB      04H             , 0dH           ; 672:  empty->687
        DB      02H             , 04H           ; 674:  MARK(4)
        DB      067H            , 09H           ; 676:  tkSColon->687
        DB      063H            , 01H           ; 678:  tkComma->681
        DB      01H                             ; 680:  Reject
        DB      02H             , 01H           ; 681:  MARK(1)
        DB      04H             , 02H           ; 683:  empty->687
        DB      02H             , 010H          ; 685:  MARK(16)
        DB      02H             , 08H           ; 687:  MARK(8)
        DB      036H            , 01H           ; 689:  IdAryElemRef->692
        DB      01H                             ; 691:  Reject
        DB      03H                             ; 692:  EMIT(opStInput)
        DW      opStInput                       
        DB      063H            , 04H           ; 695:  tkComma->701
        DB      03H                             ; 697:  EMIT(opInputEos)
        DW      opInputEos                      
        DB      00H                             ; 700:  Accept
        DB      036H            , 01H           ; 701:  IdAryElemRef->704
        DB      01H                             ; 703:  Reject
        DB      03H                             ; 704:  EMIT(opStInput)
        DW      opStInput                       
        DB      04H             , 0d2H          ; 707:  empty->695

        DB      01cH            , 01H           ; 709:  optFilenum->712
        DB      01H                             ; 711:  Reject
        DB      09H             , 01H           ; 712:  commaExp->715
        DB      01H                             ; 714:  Reject
        DB      03H                             ; 715:  EMIT(opStIoctl)
        DW      opStIoctl                       
        DB      00H                             ; 718:  Accept

        DB      0e0H, 0eH       , 022H          ; 719:  tkOFF->756
        DB      0e0H, 0fH       , 018H          ; 722:  tkON->749
        DB      0d4H            , 0fH           ; 725:  tkLIST->742
        DB      014H            , 07H           ; 727:  fn1arg->736
        DB      013H            , 01H           ; 729:  expCommaExp->732
        DB      01H                             ; 731:  Reject
        DB      03H                             ; 732:  EMIT(opStKeyMap)
        DW      opStKeyMap                      
        DB      00H                             ; 735:  Accept
        DB      03H                             ; 736:  EMIT(opEvKey)
        DW      opEvKey                         
        DB      04H             , 0e6H, 0afH    ; 739:  empty->1711
        DB      03H                             ; 742:  EMIT(opStKey)
        DW      opStKey                         
        DB      03H                             ; 745:  EMIT(2)
        DW      2
        DB      00H                             ; 748:  Accept
        DB      03H                             ; 749:  EMIT(opStKey)
        DW      opStKey                         
        DB      03H                             ; 752:  EMIT(1)
        DW      1
        DB      00H                             ; 755:  Accept
        DB      03H                             ; 756:  EMIT(opStKey)
        DW      opStKey                         
        DB      03H                             ; 759:  EMIT(0)
        DW      0
        DB      00H                             ; 762:  Accept

        DB      031H            , 01H           ; 763:  Exp->766
        DB      01H                             ; 765:  Reject
        DB      03H                             ; 766:  EMIT(opStKill)
        DW      opStKill                        
        DB      00H                             ; 769:  Accept

        DB      03H                             ; 770:  EMIT(opStLet)
        DW      opStLet                         
        DB      025H            , 0FFH          ; 773:  Assignment->Accept
        DB      01H                             ; 775:  Reject

        DB      0dH             , 00H           ; 776:  coordStep->778
        DB      064H            , 01H           ; 778:  tkMinus->781
        DB      01H                             ; 780:  Reject
        DB      0eH             , 01H           ; 781:  coord2Step->784
        DB      01H                             ; 783:  Reject
        DB      063H            , 01H           ; 784:  tkComma->787
        DB      00H                             ; 786:  Accept
        DB      031H            , 02H           ; 787:  Exp->791
        DB      04H             , 02H           ; 789:  empty->793
        DB      02H             , 01H           ; 791:  MARK(1)
        DB      063H            , 01H           ; 793:  tkComma->796
        DB      00H                             ; 795:  Accept
        DB      050H            , 0eH           ; 796:  RwBF->812
        DB      04eH            , 02H           ; 798:  RwB->802
        DB      04H             , 0cH           ; 800:  empty->814
        DB      04fH            , 04H           ; 802:  RwF->808
        DB      02H             , 02H           ; 804:  MARK(2)
        DB      04H             , 06H           ; 806:  empty->814
        DB      02H             , 03H           ; 808:  MARK(3)
        DB      04H             , 02H           ; 810:  empty->814
        DB      02H             , 03H           ; 812:  MARK(3)
        DB      09H             , 01H           ; 814:  commaExp->817
        DB      00H                             ; 816:  Accept
        DB      02H             , 04H           ; 817:  MARK(4)
        DB      00H                             ; 819:  Accept

        DB      0c4H            , 01H           ; 820:  tkINPUT->823
        DB      01H                             ; 822:  Reject
        DB      01aH            , 022H          ; 823:  lbsInpExpComma->859
        DB      067H            , 0fH           ; 825:  tkSColon->842
        DB      047H            , 02H           ; 827:  LitString->831
        DB      04H             , 01eH          ; 829:  empty->861
        DB      02H             , 04H           ; 831:  MARK(4)
        DB      067H            , 01aH          ; 833:  tkSColon->861
        DB      063H            , 01H           ; 835:  tkComma->838
        DB      01H                             ; 837:  Reject
        DB      02H             , 01H           ; 838:  MARK(1)
        DB      04H             , 013H          ; 840:  empty->861
        DB      02H             , 02H           ; 842:  MARK(2)
        DB      047H            , 02H           ; 844:  LitString->848
        DB      04H             , 0dH           ; 846:  empty->861
        DB      02H             , 04H           ; 848:  MARK(4)
        DB      067H            , 09H           ; 850:  tkSColon->861
        DB      063H            , 01H           ; 852:  tkComma->855
        DB      01H                             ; 854:  Reject
        DB      02H             , 01H           ; 855:  MARK(1)
        DB      04H             , 02H           ; 857:  empty->861
        DB      02H             , 010H          ; 859:  MARK(16)
        DB      036H            , 0FFH          ; 861:  IdAryElemRef->Accept
        DB      01H                             ; 863:  Reject

        DB      04dH            , 0FFH          ; 864:  NArgsMax5->Accept
        DB      01H                             ; 866:  Reject

        DB      01cH            , 01H           ; 867:  optFilenum->870
        DB      01H                             ; 869:  Reject
        DB      063H            , 01H           ; 870:  tkComma->873
        DB      00H                             ; 872:  Accept
        DB      031H            , 09H           ; 873:  Exp->884
        DB      0e0H, 053H      , 01H           ; 875:  tkTO->879
        DB      01H                             ; 878:  Reject
        DB      02H             , 03H           ; 879:  MARK(3)
        DB      04H             , 0eaH, 0ebH    ; 881:  empty->2795
        DB      02H             , 01H           ; 884:  MARK(1)
        DB      0e0H, 053H      , 01H           ; 886:  tkTO->890
        DB      00H                             ; 889:  Accept
        DB      02H             , 02H           ; 890:  MARK(2)
        DB      04H             , 0eaH, 0ebH    ; 892:  empty->2795

        DB      0e0H, 064H      , 012H          ; 895:  tkWHILE->916
        DB      0e0H, 05bH      , 06H           ; 898:  tkUNTIL->907
        DB      03H                             ; 901:  EMIT(opStLoop)
        DW      opStLoop                        
        DB      04H             , 0e9H, 08fH    ; 904:  empty->2447
        DB      031H            , 01H           ; 907:  Exp->910
        DB      01H                             ; 909:  Reject
        DB      03H                             ; 910:  EMIT(opStLoopUntil)
        DW      opStLoopUntil                   
        DB      04H             , 0e9H, 08fH    ; 913:  empty->2447
        DB      031H            , 01H           ; 916:  Exp->919
        DB      01H                             ; 918:  Reject
        DB      03H                             ; 919:  EMIT(opStLoopWhile)
        DW      opStLoopWhile                   
        DB      04H             , 0e9H, 08fH    ; 922:  empty->2447

        DB      03H                             ; 925:  EMIT(opStLprint)
        DW      opStLprint                      
        DB      04H             , 0e5H, 0eH     ; 928:  empty->1294

        DB      02H             , 01H           ; 931:  MARK(1)
        DB      036H            , 01H           ; 933:  IdAryElemRef->936
        DB      01H                             ; 935:  Reject
        DB      02H             , 02H           ; 936:  MARK(2)
        DB      04H             , 0e5H, 0cfH    ; 938:  empty->1487

        DB      05fH            , 01H           ; 941:  tkLParen->944
        DB      01H                             ; 943:  Reject
        DB      02H             , 01H           ; 944:  MARK(1)
        DB      036H            , 01H           ; 946:  IdAryElemRef->949
        DB      01H                             ; 948:  Reject
        DB      02H             , 02H           ; 949:  MARK(2)
        DB      012H            , 01H           ; 951:  exp12->954
        DB      01H                             ; 953:  Reject
        DB      060H            , 0e5H, 0cfH    ; 954:  tkRParen->1487
        DB      01H                             ; 957:  Reject

        DB      031H            , 01H           ; 958:  Exp->961
        DB      01H                             ; 960:  Reject
        DB      03H                             ; 961:  EMIT(opStMkdir)
        DW      opStMkdir                       
        DB      00H                             ; 964:  Accept

        DB      031H            , 01H           ; 965:  Exp->968
        DB      01H                             ; 967:  Reject
        DB      072H            , 01H           ; 968:  tkAS->971
        DB      01H                             ; 970:  Reject
        DB      031H            , 01H           ; 971:  Exp->974
        DB      01H                             ; 973:  Reject
        DB      03H                             ; 974:  EMIT(opStName)
        DW      opStName                        
        DB      00H                             ; 977:  Accept

        DB      03bH            , 07H           ; 978:  IdFor->987
        DB      03H                             ; 980:  EMIT(opStNext)
        DW      opStNext                        
        DB      0fH             , 0e9H, 08fH    ; 983:  EMITFFFF->2447
        DB      01H                             ; 986:  Reject
        DB      03H                             ; 987:  EMIT(opStNextId)
        DW      opStNextId                      
        DB      0fH             , 01H           ; 990:  EMITFFFF->993
        DB      01H                             ; 992:  Reject
        DB      0fH             , 01H           ; 993:  EMITFFFF->996
        DB      01H                             ; 995:  Reject
        DB      063H            , 01H           ; 996:  tkComma->999
        DB      00H                             ; 998:  Accept
        DB      03bH            , 01H           ; 999:  IdFor->1002
        DB      01H                             ; 1001:  Reject
        DB      03H                             ; 1002:  EMIT(opStNextId)
        DW      opStNextId                      
        DB      0fH             , 01H           ; 1005:  EMITFFFF->1008
        DB      01H                             ; 1007:  Reject
        DB      0fH             , 0d2H          ; 1008:  EMITFFFF->996
        DB      01H                             ; 1010:  Reject

        DB      010H            , 029H          ; 1011:  event->1054
        DB      0b1H            , 017H          ; 1013:  tkERROR->1038
        DB      031H            , 01H           ; 1015:  Exp->1018
        DB      01H                             ; 1017:  Reject
        DB      0beH            , 07H           ; 1018:  tkGOTO->1027
        DB      0bdH            , 01H           ; 1020:  tkGOSUB->1023
        DB      01H                             ; 1022:  Reject
        DB      02H             , 02H           ; 1023:  MARK(2)
        DB      04H             , 02H           ; 1025:  empty->1029
        DB      02H             , 01H           ; 1027:  MARK(1)
        DB      046H            , 01H           ; 1029:  LabLn->1032
        DB      01H                             ; 1031:  Reject
        DB      063H            , 01H           ; 1032:  tkComma->1035
        DB      00H                             ; 1034:  Accept
        DB      046H            , 0dbH          ; 1035:  LabLn->1032
        DB      01H                             ; 1037:  Reject
        DB      0beH            , 01H           ; 1038:  tkGOTO->1041
        DB      01H                             ; 1040:  Reject
        DB      048H            , 05H           ; 1041:  Lit0->1048
        DB      03H                             ; 1043:  EMIT(opStOnError)
        DW      opStOnError                     
        DB      04H             , 0eH           ; 1046:  empty->1062
        DB      03H                             ; 1048:  EMIT(opStOnError)
        DW      opStOnError                     
        DB      04H             , 0e9H, 08fH    ; 1051:  empty->2447
        DB      0bdH            , 01H           ; 1054:  tkGOSUB->1057
        DB      01H                             ; 1056:  Reject
        DB      048H            , 06H           ; 1057:  Lit0->1065
        DB      03H                             ; 1059:  EMIT(opEvGosub)
        DW      opEvGosub                       
        DB      046H            , 0FFH          ; 1062:  LabLn->Accept
        DB      01H                             ; 1064:  Reject
        DB      03H                             ; 1065:  EMIT(opEvGosub)
        DW      opEvGosub                       
        DB      04H             , 0e9H, 08fH    ; 1068:  empty->2447

        DB      031H            , 01H           ; 1071:  Exp->1074
        DB      01H                             ; 1073:  Reject
        DB      0b8H            , 02H           ; 1074:  tkFOR->1078
        DB      04H             , 01fH          ; 1076:  empty->1109
        DB      071H            , 01bH          ; 1078:  tkAPPEND->1107
        DB      0c4H            , 015H          ; 1080:  tkINPUT->1103
        DB      0e0H, 014H      , 0eH           ; 1082:  tkOUTPUT->1099
        DB      0e0H, 023H      , 07H           ; 1085:  tkRANDOM->1095
        DB      077H            , 01H           ; 1088:  tkBINARY->1091
        DB      01H                             ; 1090:  Reject
        DB      02H             , 05H           ; 1091:  MARK(5)
        DB      04H             , 0eH           ; 1093:  empty->1109
        DB      02H             , 04H           ; 1095:  MARK(4)
        DB      04H             , 0aH           ; 1097:  empty->1109
        DB      02H             , 03H           ; 1099:  MARK(3)
        DB      04H             , 06H           ; 1101:  empty->1109
        DB      02H             , 02H           ; 1103:  MARK(2)
        DB      04H             , 02H           ; 1105:  empty->1109
        DB      02H             , 01H           ; 1107:  MARK(1)
        DB      06dH            , 02H           ; 1109:  tkACCESS->1113
        DB      04H             , 014H          ; 1111:  empty->1133
        DB      0e0H, 025H      , 08H           ; 1113:  tkREAD->1124
        DB      0e0H, 067H      , 01H           ; 1116:  tkWRITE->1120
        DB      01H                             ; 1119:  Reject
        DB      02H             , 07H           ; 1120:  MARK(7)
        DB      04H             , 09H           ; 1122:  empty->1133
        DB      02H             , 06H           ; 1124:  MARK(6)
        DB      0e0H, 067H      , 02H           ; 1126:  tkWRITE->1131
        DB      04H             , 02H           ; 1129:  empty->1133
        DB      02H             , 08H           ; 1131:  MARK(8)
        DB      0d8H            , 09H           ; 1133:  tkLOCK->1144
        DB      0e0H, 039H      , 02H           ; 1135:  tkSHARED->1140
        DB      04H             , 018H          ; 1138:  empty->1164
        DB      02H             , 0cH           ; 1140:  MARK(12)
        DB      04H             , 014H          ; 1142:  empty->1164
        DB      0e0H, 025H      , 08H           ; 1144:  tkREAD->1155
        DB      0e0H, 067H      , 01H           ; 1147:  tkWRITE->1151
        DB      01H                             ; 1150:  Reject
        DB      02H             , 0aH           ; 1151:  MARK(10)
        DB      04H             , 09H           ; 1153:  empty->1164
        DB      0e0H, 067H      , 04H           ; 1155:  tkWRITE->1162
        DB      02H             , 09H           ; 1158:  MARK(9)
        DB      04H             , 02H           ; 1160:  empty->1164
        DB      02H             , 0bH           ; 1162:  MARK(11)
        DB      072H            , 0cH           ; 1164:  tkAS->1178
        DB      063H            , 01H           ; 1166:  tkComma->1169
        DB      01H                             ; 1168:  Reject
        DB      01cH            , 01H           ; 1169:  optFilenum->1172
        DB      01H                             ; 1171:  Reject
        DB      012H            , 01H           ; 1172:  exp12->1175
        DB      01H                             ; 1174:  Reject
        DB      02H             , 0eH           ; 1175:  MARK(14)
        DB      00H                             ; 1177:  Accept
        DB      01cH            , 01H           ; 1178:  optFilenum->1181
        DB      01H                             ; 1180:  Reject
        DB      0d1H            , 01H           ; 1181:  tkLEN->1184
        DB      00H                             ; 1183:  Accept
        DB      069H            , 01H           ; 1184:  tkEQ->1187
        DB      01H                             ; 1186:  Reject
        DB      031H            , 01H           ; 1187:  Exp->1190
        DB      01H                             ; 1189:  Reject
        DB      02H             , 0dH           ; 1190:  MARK(13)
        DB      00H                             ; 1192:  Accept

        DB      075H            , 01H           ; 1193:  tkBASE->1196
        DB      01H                             ; 1195:  Reject
        DB      048H            , 07H           ; 1196:  Lit0->1205
        DB      049H            , 01H           ; 1198:  Lit1->1201
        DB      01H                             ; 1200:  Reject
        DB      03H                             ; 1201:  EMIT(opStOptionBase1)
        DW      opStOptionBase1                 
        DB      00H                             ; 1204:  Accept
        DB      03H                             ; 1205:  EMIT(opStOptionBase0)
        DW      opStOptionBase0                 
        DB      00H                             ; 1208:  Accept

        DB      013H            , 01H           ; 1209:  expCommaExp->1212
        DB      01H                             ; 1211:  Reject
        DB      03H                             ; 1212:  EMIT(opStOut)
        DW      opStOut                         
        DB      00H                             ; 1215:  Accept

        DB      0dH             , 01H           ; 1216:  coordStep->1219
        DB      01H                             ; 1218:  Reject
        DB      0aH             , 01H           ; 1219:  commaOptExp->1222
        DB      01H                             ; 1221:  Reject
        DB      0aH             , 01H           ; 1222:  commaOptExp->1225
        DB      01H                             ; 1224:  Reject
        DB      09H             , 04H           ; 1225:  commaExp->1231
        DB      03H                             ; 1227:  EMIT(opStPaint2)
        DW      opStPaint2                      
        DB      00H                             ; 1230:  Accept
        DB      03H                             ; 1231:  EMIT(opStPaint3)
        DW      opStPaint3                      
        DB      00H                             ; 1234:  Accept

        DB      0e0H, 05cH      , 0aH           ; 1235:  tkUSING->1248
        DB      013H            , 04H           ; 1238:  expCommaExp->1244
        DB      03H                             ; 1240:  EMIT(opStPalette0)
        DW      opStPalette0                    
        DB      00H                             ; 1243:  Accept
        DB      03H                             ; 1244:  EMIT(opStPalette2)
        DW      opStPalette2                    
        DB      00H                             ; 1247:  Accept
        DB      037H            , 01H           ; 1248:  IdAryGetPut->1251
        DB      01H                             ; 1250:  Reject
        DB      03H                             ; 1251:  EMIT(opStPaletteUsing)
        DW      opStPaletteUsing                
        DB      00H                             ; 1254:  Accept

        DB      013H            , 01H           ; 1255:  expCommaExp->1258
        DB      01H                             ; 1257:  Reject
        DB      03H                             ; 1258:  EMIT(opStPCopy)
        DW      opStPCopy                       
        DB      00H                             ; 1261:  Accept

        DB      03H                             ; 1262:  EMIT(opEvPen)
        DW      opEvPen                         
        DB      04H             , 0e6H, 0afH    ; 1265:  empty->1711

        DB      031H            , 01H           ; 1268:  Exp->1271
        DB      01H                             ; 1270:  Reject
        DB      03H                             ; 1271:  EMIT(opStPlay)
        DW      opStPlay                        
        DB      00H                             ; 1274:  Accept

        DB      03H                             ; 1275:  EMIT(opEvPlay0)
        DW      opEvPlay0                       
        DB      04H             , 0e6H, 0afH    ; 1278:  empty->1711

        DB      013H            , 01H           ; 1281:  expCommaExp->1284
        DB      01H                             ; 1283:  Reject
        DB      03H                             ; 1284:  EMIT(opStPoke)
        DW      opStPoke                        
        DB      00H                             ; 1287:  Accept

        DB      0dH             , 0eaH, 09cH    ; 1288:  coordStep->2716
        DB      01H                             ; 1291:  Reject

        DB      019H            , 00H           ; 1292:  lbsExpComma->1294
        DB      020H            , 0FFH          ; 1294:  printList->Accept
        DB      01H                             ; 1296:  Reject

        DB      01cH            , 01H           ; 1297:  optFilenum->1300
        DB      01H                             ; 1299:  Reject
        DB      063H            , 04H           ; 1300:  tkComma->1306
        DB      03H                             ; 1302:  EMIT(opStPut1)
        DW      opStPut1                        
        DB      00H                             ; 1305:  Accept
        DB      031H            , 0cH           ; 1306:  Exp->1320
        DB      063H            , 01H           ; 1308:  tkComma->1311
        DB      01H                             ; 1310:  Reject
        DB      036H            , 01H           ; 1311:  IdAryElemRef->1314
        DB      01H                             ; 1313:  Reject
        DB      03H                             ; 1314:  EMIT(opStPutRec2)
        DW      opStPutRec2                     
        DB      04H             , 0e9H, 08fH    ; 1317:  empty->2447
        DB      063H            , 04H           ; 1320:  tkComma->1326
        DB      03H                             ; 1322:  EMIT(opStPut2)
        DW      opStPut2                        
        DB      00H                             ; 1325:  Accept
        DB      036H            , 01H           ; 1326:  IdAryElemRef->1329
        DB      01H                             ; 1328:  Reject
        DB      03H                             ; 1329:  EMIT(opStPutRec3)
        DW      opStPutRec3                     
        DB      04H             , 0e9H, 08fH    ; 1332:  empty->2447

        DB      0dH             , 01H           ; 1335:  coordStep->1338
        DB      01H                             ; 1337:  Reject
        DB      063H            , 01H           ; 1338:  tkComma->1341
        DB      01H                             ; 1340:  Reject
        DB      037H            , 01H           ; 1341:  IdAryGetPut->1344
        DB      01H                             ; 1343:  Reject
        DB      03H                             ; 1344:  EMIT(opStGraphicsPut)
        DW      opStGraphicsPut                 
        DB      063H            , 03H           ; 1347:  tkComma->1352
        DB      04H             , 0e9H, 08fH    ; 1349:  empty->2447
        DB      06fH            , 01dH          ; 1352:  tkAND->1383
        DB      0e0H, 012H      , 016H          ; 1354:  tkOR->1379
        DB      0e0H, 01fH      , 0fH           ; 1357:  tkPRESET->1375
        DB      0e0H, 021H      , 08H           ; 1360:  tkPSET->1371
        DB      0e0H, 068H      , 01H           ; 1363:  tkXOR->1367
        DB      01H                             ; 1366:  Reject
        DB      03H                             ; 1367:  EMIT(4)
        DW      4
        DB      00H                             ; 1370:  Accept
        DB      03H                             ; 1371:  EMIT(3)
        DW      3
        DB      00H                             ; 1374:  Accept
        DB      03H                             ; 1375:  EMIT(2)
        DW      2
        DB      00H                             ; 1378:  Accept
        DB      03H                             ; 1379:  EMIT(0)
        DW      0
        DB      00H                             ; 1382:  Accept
        DB      03H                             ; 1383:  EMIT(1)
        DW      1
        DB      00H                             ; 1386:  Accept

        DB      031H            , 04H           ; 1387:  Exp->1393
        DB      03H                             ; 1389:  EMIT(opStRandomize0)
        DW      opStRandomize0                  
        DB      00H                             ; 1392:  Accept
        DB      03H                             ; 1393:  EMIT(opStRandomize1)
        DW      opStRandomize1                  
        DB      00H                             ; 1396:  Accept

        DB      036H            , 01H           ; 1397:  IdAryElemRef->1400
        DB      01H                             ; 1399:  Reject
        DB      03H                             ; 1400:  EMIT(opStRead)
        DW      opStRead                        
        DB      063H            , 01H           ; 1403:  tkComma->1406
        DB      00H                             ; 1405:  Accept
        DB      036H            , 01H           ; 1406:  IdAryElemRef->1409
        DB      01H                             ; 1408:  Reject
        DB      03H                             ; 1409:  EMIT(opStRead)
        DW      opStRead                        
        DB      04H             , 0d5H          ; 1412:  empty->1403

        DB      0e0H, 039H      , 02H           ; 1414:  tkSHARED->1419
        DB      04H             , 06H           ; 1417:  empty->1425
        DB      023H            , 01H           ; 1419:  ACTIONidShared->1422
        DB      01H                             ; 1421:  Reject
        DB      03H                             ; 1422:  EMIT(opShared)
        DW      opShared                        
        DB      034H            , 01H           ; 1425:  IdAryRedim->1428
        DB      01H                             ; 1427:  Reject
        DB      063H            , 01H           ; 1428:  tkComma->1431
        DB      00H                             ; 1430:  Accept
        DB      034H            , 0dbH          ; 1431:  IdAryRedim->1428
        DB      01H                             ; 1433:  Reject

        DB      03H                             ; 1434:  EMIT(opStReset)
        DW      opStReset                       
        DB      00H                             ; 1437:  Accept

        DB      02H             , 01H           ; 1438:  MARK(1)
        DB      046H            , 01H           ; 1440:  LabLn->1443
        DB      00H                             ; 1442:  Accept
        DB      02H             , 02H           ; 1443:  MARK(2)
        DB      00H                             ; 1445:  Accept

        DB      02H             , 01H           ; 1446:  MARK(1)
        DB      048H            , 0cH           ; 1448:  Lit0->1462
        DB      046H            , 07H           ; 1450:  LabLn->1459
        DB      0e0H, 0bH       , 01H           ; 1452:  tkNEXT->1456
        DB      00H                             ; 1455:  Accept
        DB      02H             , 04H           ; 1456:  MARK(4)
        DB      00H                             ; 1458:  Accept
        DB      02H             , 02H           ; 1459:  MARK(2)
        DB      00H                             ; 1461:  Accept
        DB      02H             , 03H           ; 1462:  MARK(3)
        DB      00H                             ; 1464:  Accept

        DB      02H             , 01H           ; 1465:  MARK(1)
        DB      046H            , 01H           ; 1467:  LabLn->1470
        DB      00H                             ; 1469:  Accept
        DB      02H             , 02H           ; 1470:  MARK(2)
        DB      00H                             ; 1472:  Accept

        DB      031H            , 01H           ; 1473:  Exp->1476
        DB      01H                             ; 1475:  Reject
        DB      03H                             ; 1476:  EMIT(opStRmdir)
        DW      opStRmdir                       
        DB      00H                             ; 1479:  Accept

        DB      02H             , 01H           ; 1480:  MARK(1)
        DB      036H            , 01H           ; 1482:  IdAryElemRef->1485
        DB      01H                             ; 1484:  Reject
        DB      02H             , 02H           ; 1485:  MARK(2)
        DB      069H            , 0eaH, 0ebH    ; 1487:  tkEQ->2795
        DB      01H                             ; 1490:  Reject

        DB      04aH            , 06H           ; 1491:  Ln->1499
        DB      031H            , 01H           ; 1493:  Exp->1496
        DB      00H                             ; 1495:  Accept
        DB      02H             , 02H           ; 1496:  MARK(2)
        DB      00H                             ; 1498:  Accept
        DB      02H             , 01H           ; 1499:  MARK(1)
        DB      00H                             ; 1501:  Accept

        DB      04cH            , 0FFH          ; 1502:  NArgsMax4->Accept
        DB      01H                             ; 1504:  Reject

        DB      01cH            , 01H           ; 1505:  optFilenum->1508
        DB      01H                             ; 1507:  Reject
        DB      09H             , 01H           ; 1508:  commaExp->1511
        DB      01H                             ; 1510:  Reject
        DB      03H                             ; 1511:  EMIT(opStSeek)
        DW      opStSeek                        
        DB      00H                             ; 1514:  Accept

        DB      07dH            , 01H           ; 1515:  tkCASE->1518
        DB      01H                             ; 1517:  Reject
        DB      031H            , 01H           ; 1518:  Exp->1521
        DB      01H                             ; 1520:  Reject
        DB      03H                             ; 1521:  EMIT(opStSelectCase)
        DW      opStSelectCase                  
        DB      04H             , 0e9H, 08fH    ; 1524:  empty->2447

        DB      03H                             ; 1527:  EMIT(opStShared)
        DW      opStShared                      
        DB      0fH             , 01H           ; 1530:  EMITFFFF->1533
        DB      01H                             ; 1532:  Reject
        DB      023H            , 01H           ; 1533:  ACTIONidShared->1536
        DB      01H                             ; 1535:  Reject
        DB      032H            , 01H           ; 1536:  IdAry->1539
        DB      01H                             ; 1538:  Reject
        DB      063H            , 01H           ; 1539:  tkComma->1542
        DB      00H                             ; 1541:  Accept
        DB      032H            , 0dbH          ; 1542:  IdAry->1539
        DB      01H                             ; 1544:  Reject

        DB      031H            , 0FFH          ; 1545:  Exp->Accept
        DB      00H                             ; 1547:  Accept

        DB      014H            , 01H           ; 1548:  fn1arg->1551
        DB      01H                             ; 1550:  Reject
        DB      03H                             ; 1551:  EMIT(opEvSignal)
        DW      opEvSignal                      
        DB      04H             , 0e6H, 0afH    ; 1554:  empty->1711

        DB      031H            , 04H           ; 1557:  Exp->1563
        DB      03H                             ; 1559:  EMIT(opStSleep0)
        DW      opStSleep0                      
        DB      00H                             ; 1562:  Accept
        DB      03H                             ; 1563:  EMIT(opStSleep1)
        DW      opStSleep1                      
        DB      00H                             ; 1566:  Accept

        DB      013H            , 01H           ; 1567:  expCommaExp->1570
        DB      01H                             ; 1569:  Reject
        DB      03H                             ; 1570:  EMIT(opStSound)
        DW      opStSound                       
        DB      00H                             ; 1573:  Accept

        DB      03H                             ; 1574:  EMIT(opStStatic)
        DW      opStStatic                      
        DB      0fH             , 01H           ; 1577:  EMITFFFF->1580
        DB      01H                             ; 1579:  Reject
        DB      024H            , 01H           ; 1580:  ACTIONidStatic->1583
        DB      01H                             ; 1582:  Reject
        DB      038H            , 01H           ; 1583:  IdAryI->1586
        DB      01H                             ; 1585:  Reject
        DB      063H            , 01H           ; 1586:  tkComma->1589
        DB      00H                             ; 1588:  Accept
        DB      038H            , 0dbH          ; 1589:  IdAryI->1586
        DB      01H                             ; 1591:  Reject

        DB      03H                             ; 1592:  EMIT(opStStop)
        DW      opStStop                        
        DB      03H                             ; 1595:  EMIT(opNop)
        DW      opNop                           
        DB      00H                             ; 1598:  Accept

        DB      014H            , 07H           ; 1599:  fn1arg->1608
        DB      0e0H, 0fH       , 0FFH          ; 1601:  tkON->Accept
        DB      0e0H, 0eH       , 0FFH          ; 1604:  tkOFF->Accept
        DB      01H                             ; 1607:  Reject
        DB      03H                             ; 1608:  EMIT(opEvStrig)
        DW      opEvStrig                       
        DB      04H             , 062H          ; 1611:  empty->1711

        DB      030H            , 01H           ; 1613:  ErrIfNot1st->1616
        DB      01H                             ; 1615:  Reject
        DB      043H            , 01H           ; 1616:  IdSubDef->1619
        DB      01H                             ; 1618:  Reject
        DB      02H             , 03H           ; 1619:  MARK(3)
        DB      01dH            , 01H           ; 1621:  parms->1624
        DB      01H                             ; 1623:  Reject
        DB      0e0H, 043H      , 01H           ; 1624:  tkSTATIC->1628
        DB      00H                             ; 1627:  Accept
        DB      02H             , 04H           ; 1628:  MARK(4)
        DB      00H                             ; 1630:  Accept

        DB      036H            , 01H           ; 1631:  IdAryElemRef->1634
        DB      01H                             ; 1633:  Reject
        DB      063H            , 01H           ; 1634:  tkComma->1637
        DB      01H                             ; 1636:  Reject
        DB      036H            , 01H           ; 1637:  IdAryElemRef->1640
        DB      01H                             ; 1639:  Reject
        DB      03H                             ; 1640:  EMIT(opStSwap)
        DW      opStSwap                        
        DB      04H             , 0e9H, 08fH    ; 1643:  empty->2447

        DB      03H                             ; 1646:  EMIT(opStSystem)
        DW      opStSystem                      
        DB      00H                             ; 1649:  Accept

        DB      069H            , 01H           ; 1650:  tkEQ->1653
        DB      01H                             ; 1652:  Reject
        DB      031H            , 01H           ; 1653:  Exp->1656
        DB      01H                             ; 1655:  Reject
        DB      03H                             ; 1656:  EMIT(opStTime_)
        DW      opStTime_                       
        DB      00H                             ; 1659:  Accept

        DB      03H                             ; 1660:  EMIT(opEvTimer0)
        DW      opEvTimer0                      
        DB      04H             , 02eH          ; 1663:  empty->1711

        DB      03H                             ; 1665:  EMIT(opStTroff)
        DW      opStTroff                       
        DB      00H                             ; 1668:  Accept

        DB      03H                             ; 1669:  EMIT(opStTron)
        DW      opStTron                        
        DB      00H                             ; 1672:  Accept

        DB      03H                             ; 1673:  EMIT(opStType)
        DW      opStType                        
        DB      0fH             , 0e9H, 0ddH    ; 1676:  EMITFFFF->2525
        DB      01H                             ; 1679:  Reject

        DB      01cH            , 01H           ; 1680:  optFilenum->1683
        DB      01H                             ; 1682:  Reject
        DB      063H            , 01H           ; 1683:  tkComma->1686
        DB      00H                             ; 1685:  Accept
        DB      031H            , 09H           ; 1686:  Exp->1697
        DB      0e0H, 053H      , 01H           ; 1688:  tkTO->1692
        DB      01H                             ; 1691:  Reject
        DB      02H             , 03H           ; 1692:  MARK(3)
        DB      04H             , 0eaH, 0ebH    ; 1694:  empty->2795
        DB      02H             , 01H           ; 1697:  MARK(1)
        DB      0e0H, 053H      , 01H           ; 1699:  tkTO->1703
        DB      00H                             ; 1702:  Accept
        DB      02H             , 02H           ; 1703:  MARK(2)
        DB      04H             , 0eaH, 0ebH    ; 1705:  empty->2795

        DB      03H                             ; 1708:  EMIT(opEvUEvent)
        DW      opEvUEvent                      
        DB      011H            , 0FFH          ; 1711:  evSwitch->Accept
        DB      01H                             ; 1713:  Reject

        DB      0e0H, 020H      , 02cH          ; 1714:  tkPRINT->1761
        DB      0e0H, 033H      , 016H          ; 1717:  tkSCREEN->1742
        DB      016H            , 04H           ; 1720:  fn2arg->1726
        DB      03H                             ; 1722:  EMIT(opStView0)
        DW      opStView0                       
        DB      00H                             ; 1725:  Accept
        DB      064H            , 01H           ; 1726:  tkMinus->1729
        DB      01H                             ; 1728:  Reject
        DB      016H            , 01H           ; 1729:  fn2arg->1732
        DB      01H                             ; 1731:  Reject
        DB      0aH             , 01H           ; 1732:  commaOptExp->1735
        DB      01H                             ; 1734:  Reject
        DB      0aH             , 01H           ; 1735:  commaOptExp->1738
        DB      01H                             ; 1737:  Reject
        DB      03H                             ; 1738:  EMIT(opStView)
        DW      opStView                        
        DB      00H                             ; 1741:  Accept
        DB      016H            , 01H           ; 1742:  fn2arg->1745
        DB      01H                             ; 1744:  Reject
        DB      064H            , 01H           ; 1745:  tkMinus->1748
        DB      01H                             ; 1747:  Reject
        DB      016H            , 01H           ; 1748:  fn2arg->1751
        DB      01H                             ; 1750:  Reject
        DB      0aH             , 01H           ; 1751:  commaOptExp->1754
        DB      01H                             ; 1753:  Reject
        DB      0aH             , 01H           ; 1754:  commaOptExp->1757
        DB      01H                             ; 1756:  Reject
        DB      03H                             ; 1757:  EMIT(opStViewScreen)
        DW      opStViewScreen                  
        DB      00H                             ; 1760:  Accept
        DB      031H            , 04H           ; 1761:  Exp->1767
        DB      03H                             ; 1763:  EMIT(opStViewPrint0)
        DW      opStViewPrint0                  
        DB      00H                             ; 1766:  Accept
        DB      0e0H, 053H      , 01H           ; 1767:  tkTO->1771
        DB      01H                             ; 1770:  Reject
        DB      031H            , 01H           ; 1771:  Exp->1774
        DB      01H                             ; 1773:  Reject
        DB      03H                             ; 1774:  EMIT(opStViewPrint2)
        DW      opStViewPrint2                  
        DB      00H                             ; 1777:  Accept

        DB      031H            , 01H           ; 1778:  Exp->1781
        DB      01H                             ; 1780:  Reject
        DB      012H            , 0FFH          ; 1781:  exp12->Accept
        DB      01H                             ; 1783:  Reject

        DB      03H                             ; 1784:  EMIT(opStWend)
        DW      opStWend                        
        DB      04H             , 0e9H, 08fH    ; 1787:  empty->2447

        DB      031H            , 01H           ; 1790:  Exp->1793
        DB      01H                             ; 1792:  Reject
        DB      03H                             ; 1793:  EMIT(opStWhile)
        DW      opStWhile                       
        DB      04H             , 0e9H, 08fH    ; 1796:  empty->2447

        DB      056H            , 01dH          ; 1799:  tkLbs->1830
        DB      0deH            , 014H          ; 1801:  tkLPRINT->1823
        DB      031H            , 09H           ; 1803:  Exp->1814
        DB      063H            , 01H           ; 1805:  tkComma->1808
        DB      01H                             ; 1807:  Reject
        DB      03H                             ; 1808:  EMIT(opUndef)
        DW      opUndef                         
        DB      031H            , 06H           ; 1811:  Exp->1819
        DB      01H                             ; 1813:  Reject
        DB      09H             , 03H           ; 1814:  commaExp->1819
        DB      03H                             ; 1816:  EMIT(opUndef)
        DW      opUndef                         
        DB      03H                             ; 1819:  EMIT(opStWidth2)
        DW      opStWidth2                      
        DB      00H                             ; 1822:  Accept
        DB      031H            , 01H           ; 1823:  Exp->1826
        DB      01H                             ; 1825:  Reject
        DB      03H                             ; 1826:  EMIT(opStWidthLprint)
        DW      opStWidthLprint                 
        DB      00H                             ; 1829:  Accept
        DB      031H            , 01H           ; 1830:  Exp->1833
        DB      01H                             ; 1832:  Reject
        DB      03H                             ; 1833:  EMIT(opLbs)
        DW      opLbs                           
        DB      063H            , 01H           ; 1836:  tkComma->1839
        DB      01H                             ; 1838:  Reject
        DB      031H            , 01H           ; 1839:  Exp->1842
        DB      01H                             ; 1841:  Reject
        DB      03H                             ; 1842:  EMIT(opStWidthFile)
        DW      opStWidthFile                   
        DB      00H                             ; 1845:  Accept

        DB      0e0H, 033H      , 010H          ; 1846:  tkSCREEN->1865
        DB      016H            , 04H           ; 1849:  fn2arg->1855
        DB      03H                             ; 1851:  EMIT(opStWindow0)
        DW      opStWindow0                     
        DB      00H                             ; 1854:  Accept
        DB      064H            , 01H           ; 1855:  tkMinus->1858
        DB      01H                             ; 1857:  Reject
        DB      016H            , 01H           ; 1858:  fn2arg->1861
        DB      01H                             ; 1860:  Reject
        DB      03H                             ; 1861:  EMIT(opStWindow)
        DW      opStWindow                      
        DB      00H                             ; 1864:  Accept
        DB      016H            , 01H           ; 1865:  fn2arg->1868
        DB      01H                             ; 1867:  Reject
        DB      064H            , 01H           ; 1868:  tkMinus->1871
        DB      01H                             ; 1870:  Reject
        DB      016H            , 01H           ; 1871:  fn2arg->1874
        DB      01H                             ; 1873:  Reject
        DB      03H                             ; 1874:  EMIT(opStWindowScreen)
        DW      opStWindowScreen                
        DB      00H                             ; 1877:  Accept

        DB      03H                             ; 1878:  EMIT(opStWrite)
        DW      opStWrite                       
        DB      019H            , 00H           ; 1881:  lbsExpComma->1883
        DB      031H            , 04H           ; 1883:  Exp->1889
        DB      03H                             ; 1885:  EMIT(opPrintEos)
        DW      opPrintEos                      
        DB      00H                             ; 1888:  Accept
        DB      063H            , 06H           ; 1889:  tkComma->1897
        DB      067H            , 04H           ; 1891:  tkSColon->1897
        DB      03H                             ; 1893:  EMIT(opPrintItemEos)
        DW      opPrintItemEos                  
        DB      00H                             ; 1896:  Accept
        DB      03H                             ; 1897:  EMIT(opPrintItemComma)
        DW      opPrintItemComma                
        DB      031H            , 0d3H          ; 1900:  Exp->1889
        DB      01H                             ; 1902:  Reject

        DB      014H            , 01H           ; 1903:  fn1arg->1906
        DB      01H                             ; 1905:  Reject
        DB      03H                             ; 1906:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 1909:  Accept

        DB      014H            , 01H           ; 1910:  fn1arg->1913
        DB      01H                             ; 1912:  Reject
        DB      03H                             ; 1913:  EMIT(opFnAsc)
        DW      opFnAsc                         
        DB      00H                             ; 1916:  Accept

        DB      014H            , 01H           ; 1917:  fn1arg->1920
        DB      01H                             ; 1919:  Reject
        DB      03H                             ; 1920:  EMIT(opFnAtn)
        DW      opFnAtn                         
        DB      00H                             ; 1923:  Accept

        DB      014H            , 01H           ; 1924:  fn1arg->1927
        DB      01H                             ; 1926:  Reject
        DB      03H                             ; 1927:  EMIT(opCoerce,ET_R8)
        DW      ET_R8*(OPCODE_MASK+1)+opCoerce
        DB      00H                             ; 1930:  Accept

        DB      014H            , 01H           ; 1931:  fn1arg->1934
        DB      01H                             ; 1933:  Reject
        DB      03H                             ; 1934:  EMIT(opFnChr_)
        DW      opFnChr_                        
        DB      00H                             ; 1937:  Accept

        DB      014H            , 01H           ; 1938:  fn1arg->1941
        DB      01H                             ; 1940:  Reject
        DB      03H                             ; 1941:  EMIT(opCoerce,ET_I2)
        DW      ET_I2*(OPCODE_MASK+1)+opCoerce
        DB      00H                             ; 1944:  Accept

        DB      014H            , 01H           ; 1945:  fn1arg->1948
        DB      01H                             ; 1947:  Reject
        DB      03H                             ; 1948:  EMIT(opCoerce,ET_I4)
        DW      ET_I4*(OPCODE_MASK+1)+opCoerce
        DB      00H                             ; 1951:  Accept

        DB      03H                             ; 1952:  EMIT(opFnCommand_)
        DW      opFnCommand_                    
        DB      00H                             ; 1955:  Accept

        DB      014H            , 01H           ; 1956:  fn1arg->1959
        DB      01H                             ; 1958:  Reject
        DB      03H                             ; 1959:  EMIT(opFnCos)
        DW      opFnCos                         
        DB      00H                             ; 1962:  Accept

        DB      014H            , 01H           ; 1963:  fn1arg->1966
        DB      01H                             ; 1965:  Reject
        DB      03H                             ; 1966:  EMIT(opCoerce,ET_R4)
        DW      ET_R4*(OPCODE_MASK+1)+opCoerce
        DB      00H                             ; 1969:  Accept

        DB      03H                             ; 1970:  EMIT(opFnCsrlin)
        DW      opFnCsrlin                      
        DB      00H                             ; 1973:  Accept

        DB      014H            , 01H           ; 1974:  fn1arg->1977
        DB      01H                             ; 1976:  Reject
        DB      03H                             ; 1977:  EMIT(opFnCvd)
        DW      opFnCvd                         
        DB      00H                             ; 1980:  Accept

        DB      014H            , 01H           ; 1981:  fn1arg->1984
        DB      01H                             ; 1983:  Reject
        DB      03H                             ; 1984:  EMIT(opFnCvdmbf)
        DW      opFnCvdmbf                      
        DB      00H                             ; 1987:  Accept

        DB      014H            , 01H           ; 1988:  fn1arg->1991
        DB      01H                             ; 1990:  Reject
        DB      03H                             ; 1991:  EMIT(opFnCvi)
        DW      opFnCvi                         
        DB      00H                             ; 1994:  Accept

        DB      014H            , 01H           ; 1995:  fn1arg->1998
        DB      01H                             ; 1997:  Reject
        DB      03H                             ; 1998:  EMIT(opFnCvl)
        DW      opFnCvl                         
        DB      00H                             ; 2001:  Accept

        DB      014H            , 01H           ; 2002:  fn1arg->2005
        DB      01H                             ; 2004:  Reject
        DB      03H                             ; 2005:  EMIT(opFnCvs)
        DW      opFnCvs                         
        DB      00H                             ; 2008:  Accept

        DB      014H            , 01H           ; 2009:  fn1arg->2012
        DB      01H                             ; 2011:  Reject
        DB      03H                             ; 2012:  EMIT(opFnCvsmbf)
        DW      opFnCvsmbf                      
        DB      00H                             ; 2015:  Accept

        DB      03H                             ; 2016:  EMIT(opFnDate_)
        DW      opFnDate_                       
        DB      00H                             ; 2019:  Accept

        DB      014H            , 01H           ; 2020:  fn1arg->2023
        DB      01H                             ; 2022:  Reject
        DB      03H                             ; 2023:  EMIT(opFnEnviron_)
        DW      opFnEnviron_                    
        DB      00H                             ; 2026:  Accept

        DB      014H            , 01H           ; 2027:  fn1arg->2030
        DB      01H                             ; 2029:  Reject
        DB      03H                             ; 2030:  EMIT(opFnEof)
        DW      opFnEof                         
        DB      00H                             ; 2033:  Accept

        DB      03H                             ; 2034:  EMIT(opFnErdev)
        DW      opFnErdev                       
        DB      00H                             ; 2037:  Accept

        DB      03H                             ; 2038:  EMIT(opFnErdev_)
        DW      opFnErdev_                      
        DB      00H                             ; 2041:  Accept

        DB      03H                             ; 2042:  EMIT(opFnErl)
        DW      opFnErl                         
        DB      00H                             ; 2045:  Accept

        DB      03H                             ; 2046:  EMIT(opFnErr)
        DW      opFnErr                         
        DB      00H                             ; 2049:  Accept

        DB      014H            , 01H           ; 2050:  fn1arg->2053
        DB      01H                             ; 2052:  Reject
        DB      03H                             ; 2053:  EMIT(opFnExp)
        DW      opFnExp                         
        DB      00H                             ; 2056:  Accept

        DB      016H            , 01H           ; 2057:  fn2arg->2060
        DB      01H                             ; 2059:  Reject
        DB      03H                             ; 2060:  EMIT(opFnFileattr)
        DW      opFnFileattr                    
        DB      00H                             ; 2063:  Accept

        DB      014H            , 01H           ; 2064:  fn1arg->2067
        DB      01H                             ; 2066:  Reject
        DB      03H                             ; 2067:  EMIT(opFnFix)
        DW      opFnFix                         
        DB      00H                             ; 2070:  Accept

        DB      014H            , 01H           ; 2071:  fn1arg->2074
        DB      01H                             ; 2073:  Reject
        DB      03H                             ; 2074:  EMIT(opFnFre)
        DW      opFnFre                         
        DB      00H                             ; 2077:  Accept

        DB      03H                             ; 2078:  EMIT(opFnFreefile)
        DW      opFnFreefile                    
        DB      00H                             ; 2081:  Accept

        DB      014H            , 01H           ; 2082:  fn1arg->2085
        DB      01H                             ; 2084:  Reject
        DB      03H                             ; 2085:  EMIT(opFnHex_)
        DW      opFnHex_                        
        DB      00H                             ; 2088:  Accept

        DB      03H                             ; 2089:  EMIT(opFnInkey_)
        DW      opFnInkey_                      
        DB      00H                             ; 2092:  Accept

        DB      014H            , 01H           ; 2093:  fn1arg->2096
        DB      01H                             ; 2095:  Reject
        DB      03H                             ; 2096:  EMIT(opFnInp)
        DW      opFnInp                         
        DB      00H                             ; 2099:  Accept

        DB      05fH            , 01H           ; 2100:  tkLParen->2103
        DB      01H                             ; 2102:  Reject
        DB      031H            , 01H           ; 2103:  Exp->2106
        DB      01H                             ; 2105:  Reject
        DB      063H            , 03H           ; 2106:  tkComma->2111
        DB      04H             , 0eaH, 0fcH    ; 2108:  empty->2812
        DB      01cH            , 0eaH, 0fcH    ; 2111:  optFilenum->2812
        DB      01H                             ; 2114:  Reject

        DB      017H            , 0FFH          ; 2115:  fn23arg->Accept
        DB      01H                             ; 2117:  Reject

        DB      014H            , 01H           ; 2118:  fn1arg->2121
        DB      01H                             ; 2120:  Reject
        DB      03H                             ; 2121:  EMIT(opFnInt)
        DW      opFnInt                         
        DB      00H                             ; 2124:  Accept

        DB      05fH            , 01H           ; 2125:  tkLParen->2128
        DB      01H                             ; 2127:  Reject
        DB      01cH            , 01H           ; 2128:  optFilenum->2131
        DB      01H                             ; 2130:  Reject
        DB      060H            , 01H           ; 2131:  tkRParen->2134
        DB      01H                             ; 2133:  Reject
        DB      03H                             ; 2134:  EMIT(opFnIoctl_)
        DW      opFnIoctl_                      
        DB      00H                             ; 2137:  Accept

        DB      018H            , 0FFH          ; 2138:  fnBoundArg->Accept
        DB      01H                             ; 2140:  Reject

        DB      014H            , 01H           ; 2141:  fn1arg->2144
        DB      01H                             ; 2143:  Reject
        DB      03H                             ; 2144:  EMIT(opFnLcase_)
        DW      opFnLcase_                      
        DB      00H                             ; 2147:  Accept

        DB      016H            , 01H           ; 2148:  fn2arg->2151
        DB      01H                             ; 2150:  Reject
        DB      03H                             ; 2151:  EMIT(opFnLeft_)
        DW      opFnLeft_                       
        DB      00H                             ; 2154:  Accept

        DB      014H            , 01H           ; 2155:  fn1arg->2158
        DB      01H                             ; 2157:  Reject
        DB      03H                             ; 2158:  EMIT(opFnLen)
        DW      opFnLen                         
        DB      04H             , 0e9H, 08fH    ; 2161:  empty->2447

        DB      014H            , 01H           ; 2164:  fn1arg->2167
        DB      01H                             ; 2166:  Reject
        DB      03H                             ; 2167:  EMIT(opFnLoc)
        DW      opFnLoc                         
        DB      00H                             ; 2170:  Accept

        DB      014H            , 01H           ; 2171:  fn1arg->2174
        DB      01H                             ; 2173:  Reject
        DB      03H                             ; 2174:  EMIT(opFnLof)
        DW      opFnLof                         
        DB      00H                             ; 2177:  Accept

        DB      014H            , 01H           ; 2178:  fn1arg->2181
        DB      01H                             ; 2180:  Reject
        DB      03H                             ; 2181:  EMIT(opFnLog)
        DW      opFnLog                         
        DB      00H                             ; 2184:  Accept

        DB      014H            , 01H           ; 2185:  fn1arg->2188
        DB      01H                             ; 2187:  Reject
        DB      03H                             ; 2188:  EMIT(opFnLpos)
        DW      opFnLpos                        
        DB      00H                             ; 2191:  Accept

        DB      014H            , 01H           ; 2192:  fn1arg->2195
        DB      01H                             ; 2194:  Reject
        DB      03H                             ; 2195:  EMIT(opFnLtrim_)
        DW      opFnLtrim_                      
        DB      00H                             ; 2198:  Accept

        DB      014H            , 01H           ; 2199:  fn1arg->2202
        DB      01H                             ; 2201:  Reject
        DB      03H                             ; 2202:  EMIT(opFnMkd_)
        DW      opFnMkd_                        
        DB      00H                             ; 2205:  Accept

        DB      014H            , 01H           ; 2206:  fn1arg->2209
        DB      01H                             ; 2208:  Reject
        DB      03H                             ; 2209:  EMIT(opFnMkdmbf_)
        DW      opFnMkdmbf_                     
        DB      00H                             ; 2212:  Accept

        DB      014H            , 01H           ; 2213:  fn1arg->2216
        DB      01H                             ; 2215:  Reject
        DB      03H                             ; 2216:  EMIT(opFnMki_)
        DW      opFnMki_                        
        DB      00H                             ; 2219:  Accept

        DB      014H            , 01H           ; 2220:  fn1arg->2223
        DB      01H                             ; 2222:  Reject
        DB      03H                             ; 2223:  EMIT(opFnMkl_)
        DW      opFnMkl_                        
        DB      00H                             ; 2226:  Accept

        DB      014H            , 01H           ; 2227:  fn1arg->2230
        DB      01H                             ; 2229:  Reject
        DB      03H                             ; 2230:  EMIT(opFnMks_)
        DW      opFnMks_                        
        DB      00H                             ; 2233:  Accept

        DB      014H            , 01H           ; 2234:  fn1arg->2237
        DB      01H                             ; 2236:  Reject
        DB      03H                             ; 2237:  EMIT(opFnMksmbf_)
        DW      opFnMksmbf_                     
        DB      00H                             ; 2240:  Accept

        DB      014H            , 01H           ; 2241:  fn1arg->2244
        DB      01H                             ; 2243:  Reject
        DB      03H                             ; 2244:  EMIT(opFnOct_)
        DW      opFnOct_                        
        DB      00H                             ; 2247:  Accept

        DB      014H            , 01H           ; 2248:  fn1arg->2251
        DB      01H                             ; 2250:  Reject
        DB      03H                             ; 2251:  EMIT(opFnPeek)
        DW      opFnPeek                        
        DB      00H                             ; 2254:  Accept

        DB      014H            , 01H           ; 2255:  fn1arg->2258
        DB      01H                             ; 2257:  Reject
        DB      03H                             ; 2258:  EMIT(opFnPen)
        DW      opFnPen                         
        DB      00H                             ; 2261:  Accept

        DB      014H            , 01H           ; 2262:  fn1arg->2265
        DB      01H                             ; 2264:  Reject
        DB      03H                             ; 2265:  EMIT(opFnPlay)
        DW      opFnPlay                        
        DB      00H                             ; 2268:  Accept

        DB      016H            , 01H           ; 2269:  fn2arg->2272
        DB      01H                             ; 2271:  Reject
        DB      03H                             ; 2272:  EMIT(opFnPmap)
        DW      opFnPmap                        
        DB      00H                             ; 2275:  Accept

        DB      015H            , 0FFH          ; 2276:  fn12arg->Accept
        DB      01H                             ; 2278:  Reject

        DB      014H            , 01H           ; 2279:  fn1arg->2282
        DB      01H                             ; 2281:  Reject
        DB      03H                             ; 2282:  EMIT(opFnPos)
        DW      opFnPos                         
        DB      00H                             ; 2285:  Accept

        DB      016H            , 01H           ; 2286:  fn2arg->2289
        DB      01H                             ; 2288:  Reject
        DB      03H                             ; 2289:  EMIT(opFnRight_)
        DW      opFnRight_                      
        DB      00H                             ; 2292:  Accept

        DB      014H            , 0FFH          ; 2293:  fn1arg->Accept
        DB      00H                             ; 2295:  Accept

        DB      014H            , 01H           ; 2296:  fn1arg->2299
        DB      01H                             ; 2298:  Reject
        DB      03H                             ; 2299:  EMIT(opFnRtrim_)
        DW      opFnRtrim_                      
        DB      00H                             ; 2302:  Accept

        DB      05fH            , 01H           ; 2303:  tkLParen->2306
        DB      01H                             ; 2305:  Reject
        DB      036H            , 01H           ; 2306:  IdAryElemRef->2309
        DB      01H                             ; 2308:  Reject
        DB      060H            , 01H           ; 2309:  tkRParen->2312
        DB      01H                             ; 2311:  Reject
        DB      03H                             ; 2312:  EMIT(opFnSadd)
        DW      opFnSadd                        
        DB      00H                             ; 2315:  Accept

        DB      014H            , 01H           ; 2316:  fn1arg->2319
        DB      01H                             ; 2318:  Reject
        DB      03H                             ; 2319:  EMIT(opFnSeek)
        DW      opFnSeek                        
        DB      00H                             ; 2322:  Accept

        DB      014H            , 01H           ; 2323:  fn1arg->2326
        DB      01H                             ; 2325:  Reject
        DB      03H                             ; 2326:  EMIT(opFnSetmem)
        DW      opFnSetmem                      
        DB      00H                             ; 2329:  Accept

        DB      014H            , 01H           ; 2330:  fn1arg->2333
        DB      01H                             ; 2332:  Reject
        DB      03H                             ; 2333:  EMIT(opFnSgn)
        DW      opFnSgn                         
        DB      00H                             ; 2336:  Accept

        DB      014H            , 01H           ; 2337:  fn1arg->2340
        DB      01H                             ; 2339:  Reject
        DB      03H                             ; 2340:  EMIT(opFnShell)
        DW      opFnShell                       
        DB      00H                             ; 2343:  Accept

        DB      014H            , 01H           ; 2344:  fn1arg->2347
        DB      01H                             ; 2346:  Reject
        DB      03H                             ; 2347:  EMIT(opFnSin)
        DW      opFnSin                         
        DB      00H                             ; 2350:  Accept

        DB      014H            , 01H           ; 2351:  fn1arg->2354
        DB      01H                             ; 2353:  Reject
        DB      03H                             ; 2354:  EMIT(opFnSpace_)
        DW      opFnSpace_                      
        DB      00H                             ; 2357:  Accept

        DB      014H            , 01H           ; 2358:  fn1arg->2361
        DB      01H                             ; 2360:  Reject
        DB      03H                             ; 2361:  EMIT(opFnSqr)
        DW      opFnSqr                         
        DB      00H                             ; 2364:  Accept

        DB      014H            , 01H           ; 2365:  fn1arg->2368
        DB      01H                             ; 2367:  Reject
        DB      03H                             ; 2368:  EMIT(opFnStick)
        DW      opFnStick                       
        DB      00H                             ; 2371:  Accept

        DB      014H            , 01H           ; 2372:  fn1arg->2375
        DB      01H                             ; 2374:  Reject
        DB      03H                             ; 2375:  EMIT(opFnStr_)
        DW      opFnStr_                        
        DB      00H                             ; 2378:  Accept

        DB      014H            , 01H           ; 2379:  fn1arg->2382
        DB      01H                             ; 2381:  Reject
        DB      03H                             ; 2382:  EMIT(opFnStrig)
        DW      opFnStrig                       
        DB      00H                             ; 2385:  Accept

        DB      016H            , 01H           ; 2386:  fn2arg->2389
        DB      01H                             ; 2388:  Reject
        DB      03H                             ; 2389:  EMIT(opFnString_)
        DW      opFnString_                     
        DB      00H                             ; 2392:  Accept

        DB      014H            , 01H           ; 2393:  fn1arg->2396
        DB      01H                             ; 2395:  Reject
        DB      03H                             ; 2396:  EMIT(opFnTan)
        DW      opFnTan                         
        DB      00H                             ; 2399:  Accept

        DB      03H                             ; 2400:  EMIT(opFnTime_)
        DW      opFnTime_                       
        DB      00H                             ; 2403:  Accept

        DB      03H                             ; 2404:  EMIT(opFnTimer)
        DW      opFnTimer                       
        DB      00H                             ; 2407:  Accept

        DB      014H            , 01H           ; 2408:  fn1arg->2411
        DB      01H                             ; 2410:  Reject
        DB      03H                             ; 2411:  EMIT(opFnUcase_)
        DW      opFnUcase_                      
        DB      00H                             ; 2414:  Accept

        DB      014H            , 01H           ; 2415:  fn1arg->2418
        DB      01H                             ; 2417:  Reject
        DB      03H                             ; 2418:  EMIT(opFnVal)
        DW      opFnVal                         
        DB      00H                             ; 2421:  Accept

        DB      05fH            , 01H           ; 2422:  tkLParen->2425
        DB      01H                             ; 2424:  Reject
        DB      036H            , 01H           ; 2425:  IdAryElemRef->2428
        DB      01H                             ; 2427:  Reject
        DB      060H            , 01H           ; 2428:  tkRParen->2431
        DB      01H                             ; 2430:  Reject
        DB      03H                             ; 2431:  EMIT(opFnVarptr)
        DW      opFnVarptr                      
        DB      00H                             ; 2434:  Accept

        DB      05fH            , 01H           ; 2435:  tkLParen->2438
        DB      01H                             ; 2437:  Reject
        DB      036H            , 01H           ; 2438:  IdAryElemRef->2441
        DB      01H                             ; 2440:  Reject
        DB      060H            , 01H           ; 2441:  tkRParen->2444
        DB      01H                             ; 2443:  Reject
        DB      03H                             ; 2444:  EMIT(opFnVarptr_)
        DW      opFnVarptr_                     
        DB      0fH             , 0FFH          ; 2447:  EMITFFFF->Accept
        DB      01H                             ; 2449:  Reject

        DB      05fH            , 01H           ; 2450:  tkLParen->2453
        DB      01H                             ; 2452:  Reject
        DB      036H            , 01H           ; 2453:  IdAryElemRef->2456
        DB      01H                             ; 2455:  Reject
        DB      060H            , 01H           ; 2456:  tkRParen->2459
        DB      01H                             ; 2458:  Reject
        DB      03H                             ; 2459:  EMIT(opFnVarseg)
        DW      opFnVarseg                      
        DB      00H                             ; 2462:  Accept

        DB      0c8H            , 027H          ; 2463:  tkINTEGER->2504
        DB      0dbH            , 01eH          ; 2465:  tkLONG->2497
        DB      0e0H, 03dH      , 014H          ; 2467:  tkSINGLE->2490
        DB      0a2H            , 0bH           ; 2470:  tkDOUBLE->2483
        DB      0e0H, 049H      , 01H           ; 2472:  tkSTRING->2476
        DB      01H                             ; 2475:  Reject
        DB      03H                             ; 2476:  EMIT(opAsTypeExp)
        DW      opAsTypeExp                     
        DB      03H                             ; 2479:  EMIT(ET_SD)
        DW      ET_SD                           
        DB      00H                             ; 2482:  Accept
        DB      03H                             ; 2483:  EMIT(opAsTypeExp)
        DW      opAsTypeExp                     
        DB      03H                             ; 2486:  EMIT(ET_R8)
        DW      ET_R8                           
        DB      00H                             ; 2489:  Accept
        DB      03H                             ; 2490:  EMIT(opAsTypeExp)
        DW      opAsTypeExp                     
        DB      03H                             ; 2493:  EMIT(ET_R4)
        DW      ET_R4                           
        DB      00H                             ; 2496:  Accept
        DB      03H                             ; 2497:  EMIT(opAsTypeExp)
        DW      opAsTypeExp                     
        DB      03H                             ; 2500:  EMIT(ET_I4)
        DW      ET_I4                           
        DB      00H                             ; 2503:  Accept
        DB      03H                             ; 2504:  EMIT(opAsTypeExp)
        DW      opAsTypeExp                     
        DB      03H                             ; 2507:  EMIT(ET_I2)
        DW      ET_I2                           
        DB      00H                             ; 2510:  Accept

        DB      05H             , 0FFH          ; 2511:  AsClausePrim->Accept
        DB      03H                             ; 2513:  EMIT(opAsType)
        DW      opAsType                        
        DB      04H             , 07H           ; 2516:  empty->2525

        DB      05H             , 0FFH          ; 2518:  AsClausePrim->Accept
        DB      070H            , 06H           ; 2520:  tkANY->2528
        DB      03H                             ; 2522:  EMIT(opAsType)
        DW      opAsType                        
        DB      03fH            , 0FFH          ; 2525:  IdType->Accept
        DB      01H                             ; 2527:  Reject
        DB      03H                             ; 2528:  EMIT(opAsTypeExp)
        DW      opAsTypeExp                     
        DB      03H                             ; 2531:  EMIT(ET_IMP)
        DW      ET_IMP                          
        DB      00H                             ; 2534:  Accept

        DB      031H            , 05H           ; 2535:  Exp->2542
        DB      0cbH            , 00H           ; 2537:  tkIS->2539
        DB      026H            , 0FFH          ; 2539:  CaseRelation->Accept
        DB      01H                             ; 2541:  Reject
        DB      0e0H, 053H      , 04H           ; 2542:  tkTO->2549
        DB      03H                             ; 2545:  EMIT(opStCase)
        DW      opStCase                        
        DB      00H                             ; 2548:  Accept
        DB      031H            , 01H           ; 2549:  Exp->2552
        DB      01H                             ; 2551:  Reject
        DB      03H                             ; 2552:  EMIT(opStCaseTo)
        DW      opStCaseTo                      
        DB      00H                             ; 2555:  Accept

        DB      0bH             , 0FFH          ; 2556:  commaOptExpNil->Accept
        DB      03H                             ; 2558:  EMIT(opUndef)
        DW      opUndef                         
        DB      00H                             ; 2561:  Accept

        DB      027H            , 01H           ; 2562:  CommaNoEos->2565
        DB      01H                             ; 2564:  Reject
        DB      031H            , 0FFH          ; 2565:  Exp->Accept
        DB      03H                             ; 2567:  EMIT(opUndef)
        DW      opUndef                         
        DB      00H                             ; 2570:  Accept

        DB      027H            , 01H           ; 2571:  CommaNoEos->2574
        DB      01H                             ; 2573:  Reject
        DB      031H            , 0FFH          ; 2574:  Exp->Accept
        DB      03H                             ; 2576:  EMIT(opNull)
        DW      opNull                          
        DB      00H                             ; 2579:  Accept

        DB      0e0H, 044H      , 07H           ; 2580:  tkSTEP->2590
        DB      016H            , 01H           ; 2583:  fn2arg->2586
        DB      01H                             ; 2585:  Reject
        DB      03H                             ; 2586:  EMIT(opCoord)
        DW      opCoord                         
        DB      00H                             ; 2589:  Accept
        DB      016H            , 01H           ; 2590:  fn2arg->2593
        DB      01H                             ; 2592:  Reject
        DB      03H                             ; 2593:  EMIT(opCoordStep)
        DW      opCoordStep                     
        DB      00H                             ; 2596:  Accept

        DB      0e0H, 044H      , 07H           ; 2597:  tkSTEP->2607
        DB      016H            , 01H           ; 2600:  fn2arg->2603
        DB      01H                             ; 2602:  Reject
        DB      03H                             ; 2603:  EMIT(opCoordSecond)
        DW      opCoordSecond                   
        DB      00H                             ; 2606:  Accept
        DB      016H            , 01H           ; 2607:  fn2arg->2610
        DB      01H                             ; 2609:  Reject
        DB      03H                             ; 2610:  EMIT(opCoordStepSecond)
        DW      opCoordStepSecond               
        DB      00H                             ; 2613:  Accept

        DB      03H                             ; 2614:  EMIT(UNDEFINED)
        DW      UNDEFINED                       
        DB      00H                             ; 2617:  Accept

        DB      08aH            , 040H          ; 2618:  tkCOM->2684
        DB      0ccH            , 037H          ; 2620:  tkKEY->2677
        DB      0e0H, 019H      , 030H          ; 2622:  tkPEN->2673
        DB      0e0H, 01aH      , 026H          ; 2625:  tkPLAY->2666
        DB      0e0H, 03bH      , 01cH          ; 2628:  tkSIGNAL->2659
        DB      0e0H, 048H      , 012H          ; 2631:  tkSTRIG->2652
        DB      0e0H, 052H      , 08H           ; 2634:  tkTIMER->2645
        DB      0e0H, 059H      , 01H           ; 2637:  tkUEVENT->2641
        DB      01H                             ; 2640:  Reject
        DB      03H                             ; 2641:  EMIT(opEvUEvent)
        DW      opEvUEvent                      
        DB      00H                             ; 2644:  Accept
        DB      014H            , 01H           ; 2645:  fn1arg->2648
        DB      01H                             ; 2647:  Reject
        DB      03H                             ; 2648:  EMIT(opEvTimer1)
        DW      opEvTimer1                      
        DB      00H                             ; 2651:  Accept
        DB      014H            , 01H           ; 2652:  fn1arg->2655
        DB      01H                             ; 2654:  Reject
        DB      03H                             ; 2655:  EMIT(opEvStrig)
        DW      opEvStrig                       
        DB      00H                             ; 2658:  Accept
        DB      014H            , 01H           ; 2659:  fn1arg->2662
        DB      01H                             ; 2661:  Reject
        DB      03H                             ; 2662:  EMIT(opEvSignal)
        DW      opEvSignal                      
        DB      00H                             ; 2665:  Accept
        DB      014H            , 01H           ; 2666:  fn1arg->2669
        DB      01H                             ; 2668:  Reject
        DB      03H                             ; 2669:  EMIT(opEvPlay1)
        DW      opEvPlay1                       
        DB      00H                             ; 2672:  Accept
        DB      03H                             ; 2673:  EMIT(opEvPen)
        DW      opEvPen                         
        DB      00H                             ; 2676:  Accept
        DB      014H            , 01H           ; 2677:  fn1arg->2680
        DB      01H                             ; 2679:  Reject
        DB      03H                             ; 2680:  EMIT(opEvKey)
        DW      opEvKey                         
        DB      00H                             ; 2683:  Accept
        DB      014H            , 01H           ; 2684:  fn1arg->2687
        DB      01H                             ; 2686:  Reject
        DB      03H                             ; 2687:  EMIT(opEvCom)
        DW      opEvCom                         
        DB      00H                             ; 2690:  Accept

        DB      0e0H, 0fH       , 0fH           ; 2691:  tkON->2709
        DB      0e0H, 0eH       , 08H           ; 2694:  tkOFF->2705
        DB      0e0H, 046H      , 01H           ; 2697:  tkSTOP->2701
        DB      01H                             ; 2700:  Reject
        DB      03H                             ; 2701:  EMIT(opEvStop)
        DW      opEvStop                        
        DB      00H                             ; 2704:  Accept
        DB      03H                             ; 2705:  EMIT(opEvOff)
        DW      opEvOff                         
        DB      00H                             ; 2708:  Accept
        DB      03H                             ; 2709:  EMIT(opEvOn)
        DW      opEvOn                          
        DB      00H                             ; 2712:  Accept

        DB      09H             , 01H           ; 2713:  commaExp->2716
        DB      01H                             ; 2715:  Reject
        DB      01bH            , 0FFH          ; 2716:  optCommaExp->Accept
        DB      01H                             ; 2718:  Reject

        DB      031H            , 01H           ; 2719:  Exp->2722
        DB      01H                             ; 2721:  Reject

        DB      063H            , 047H          ; 2722:  tkComma->2795
        DB      01H                             ; 2724:  Reject

        DB      05fH            , 01H           ; 2725:  tkLParen->2728
        DB      01H                             ; 2727:  Reject
        DB      031H            , 052H          ; 2728:  Exp->2812
        DB      01H                             ; 2730:  Reject

        DB      05fH            , 01H           ; 2731:  tkLParen->2734
        DB      01H                             ; 2733:  Reject
        DB      031H            , 016H          ; 2734:  Exp->2758
        DB      01H                             ; 2736:  Reject

        DB      05fH            , 01H           ; 2737:  tkLParen->2740
        DB      01H                             ; 2739:  Reject
        DB      013H            , 046H          ; 2740:  expCommaExp->2812
        DB      01H                             ; 2742:  Reject

        DB      05fH            , 01H           ; 2743:  tkLParen->2746
        DB      01H                             ; 2745:  Reject
        DB      031H            , 01H           ; 2746:  Exp->2749
        DB      01H                             ; 2748:  Reject
        DB      012H            , 03dH          ; 2749:  exp12->2812
        DB      01H                             ; 2751:  Reject

        DB      05fH            , 01H           ; 2752:  tkLParen->2755
        DB      01H                             ; 2754:  Reject
        DB      039H            , 01H           ; 2755:  IdArray->2758
        DB      01H                             ; 2757:  Reject
        DB      01bH            , 034H          ; 2758:  optCommaExp->2812
        DB      01H                             ; 2760:  Reject

        DB      056H            , 01H           ; 2761:  tkLbs->2764
        DB      01H                             ; 2763:  Reject
        DB      031H            , 01H           ; 2764:  Exp->2767
        DB      01H                             ; 2766:  Reject
        DB      03H                             ; 2767:  EMIT(opLbs)
        DW      opLbs                           
        DB      03H                             ; 2770:  EMIT(opChanOut)
        DW      opChanOut                       
        DB      04H             , 0cH           ; 2773:  empty->2787

        DB      056H            , 01H           ; 2775:  tkLbs->2778
        DB      01H                             ; 2777:  Reject
        DB      031H            , 01H           ; 2778:  Exp->2781
        DB      01H                             ; 2780:  Reject
        DB      03H                             ; 2781:  EMIT(opLbs)
        DW      opLbs                           
        DB      03H                             ; 2784:  EMIT(opInputChan)
        DW      opInputChan                     
        DB      063H            , 0FFH          ; 2787:  tkComma->Accept
        DB      01H                             ; 2789:  Reject

        DB      09H             , 0FFH          ; 2790:  commaExp->Accept
        DB      00H                             ; 2792:  Accept

        DB      056H            , 03H           ; 2793:  tkLbs->2798
        DB      031H            , 0FFH          ; 2795:  Exp->Accept
        DB      01H                             ; 2797:  Reject
        DB      031H            , 01H           ; 2798:  Exp->2801
        DB      01H                             ; 2800:  Reject
        DB      03H                             ; 2801:  EMIT(opLbs)
        DW      opLbs                           
        DB      00H                             ; 2804:  Accept

        DB      05fH            , 01H           ; 2805:  tkLParen->2808
        DB      00H                             ; 2807:  Accept
        DB      02H             , 06H           ; 2808:  MARK(6)
        DB      041H            , 0bH           ; 2810:  IdParm->2823
        DB      060H            , 0FFH          ; 2812:  tkRParen->Accept
        DB      01H                             ; 2814:  Reject

        DB      05fH            , 01H           ; 2815:  tkLParen->2818
        DB      00H                             ; 2817:  Accept
        DB      02H             , 06H           ; 2818:  MARK(6)
        DB      041H            , 01H           ; 2820:  IdParm->2823
        DB      01H                             ; 2822:  Reject
        DB      063H            , 03H           ; 2823:  tkComma->2828
        DB      060H            , 0FFH          ; 2825:  tkRParen->Accept
        DB      01H                             ; 2827:  Reject
        DB      041H            , 0d9H          ; 2828:  IdParm->2823
        DB      01H                             ; 2830:  Reject

        DB      02eH            , 0FFH          ; 2831:  EndPrint->Accept
        DB      0e0H, 04eH      , 027H          ; 2833:  tkTAB->2875
        DB      0e0H, 041H      , 01dH          ; 2836:  tkSPC->2868
        DB      063H            , 017H          ; 2839:  tkComma->2864
        DB      067H            , 011H          ; 2841:  tkSColon->2860
        DB      031H            , 01H           ; 2843:  Exp->2846
        DB      01H                             ; 2845:  Reject
        DB      063H            , 08H           ; 2846:  tkComma->2856
        DB      067H            , 02H           ; 2848:  tkSColon->2852
        DB      04H             , 03fH          ; 2850:  empty->2915
        DB      03H                             ; 2852:  EMIT(opPrintItemSemi)
        DW      opPrintItemSemi                 
        DB      00H                             ; 2855:  Accept
        DB      03H                             ; 2856:  EMIT(opPrintItemComma)
        DW      opPrintItemComma                
        DB      00H                             ; 2859:  Accept
        DB      03H                             ; 2860:  EMIT(opPrintSemi)
        DW      opPrintSemi                     
        DB      00H                             ; 2863:  Accept
        DB      03H                             ; 2864:  EMIT(opPrintComma)
        DW      opPrintComma                    
        DB      00H                             ; 2867:  Accept
        DB      014H            , 01H           ; 2868:  fn1arg->2871
        DB      01H                             ; 2870:  Reject
        DB      03H                             ; 2871:  EMIT(opPrintSpc)
        DW      opPrintSpc                      
        DB      00H                             ; 2874:  Accept
        DB      014H            , 01H           ; 2875:  fn1arg->2878
        DB      01H                             ; 2877:  Reject
        DB      03H                             ; 2878:  EMIT(opPrintTab)
        DW      opPrintTab                      
        DB      00H                             ; 2881:  Accept

        DB      01fH            , 0deH          ; 2882:  printItem->2882
        DB      0e0H, 05cH      , 01H           ; 2884:  tkUSING->2888
        DB      00H                             ; 2887:  Accept
        DB      031H            , 01H           ; 2888:  Exp->2891
        DB      01H                             ; 2890:  Reject
        DB      03H                             ; 2891:  EMIT(opUsing)
        DW      opUsing                         
        DB      067H            , 01H           ; 2894:  tkSColon->2897
        DB      01H                             ; 2896:  Reject
        DB      021H            , 0deH          ; 2897:  printUsingItem->2897
        DB      00H                             ; 2899:  Accept

        DB      02eH            , 0FFH          ; 2900:  EndPrint->Accept
        DB      0e0H, 04eH      , 019H          ; 2902:  tkTAB->2930
        DB      0e0H, 041H      , 0eH           ; 2905:  tkSPC->2922
        DB      031H            , 01H           ; 2908:  Exp->2911
        DB      01H                             ; 2910:  Reject
        DB      063H            , 05H           ; 2911:  tkComma->2918
        DB      067H            , 03H           ; 2913:  tkSColon->2918
        DB      02fH            , 0FFH          ; 2915:  EndPrintExp->Accept
        DB      01H                             ; 2917:  Reject
        DB      03H                             ; 2918:  EMIT(opPrintItemSemi)
        DW      opPrintItemSemi                 
        DB      00H                             ; 2921:  Accept
        DB      014H            , 01H           ; 2922:  fn1arg->2925
        DB      01H                             ; 2924:  Reject
        DB      03H                             ; 2925:  EMIT(opPrintSpc)
        DW      opPrintSpc                      
        DB      04H             , 06H           ; 2928:  empty->2936
        DB      014H            , 01H           ; 2930:  fn1arg->2933
        DB      01H                             ; 2932:  Reject
        DB      03H                             ; 2933:  EMIT(opPrintTab)
        DW      opPrintTab                      
        DB      067H            , 0FFH          ; 2936:  tkSColon->Accept
        DB      063H            , 0FFH          ; 2938:  tkComma->Accept
        DB      00H                             ; 2940:  Accept

; state table = 2941 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
