        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 10:03:51 2026


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
        DW      2504    ; AsClausePrim
        DW      2552    ; AsClause
        DW      2560    ; AsClauseAny
        DW      2577    ; caseItem
        DW      2598    ; commaExp
        DW      2604    ; commaOptExp
        DW      2610    ; commaOptExpNil
        DW      2619    ; commaOptExpNull
        DW      2628    ; coordStep
        DW      2645    ; coord2Step
        DW      2662    ; EMITFFFF
        DW      2666    ; event
        DW      2739    ; evSwitch
        DW      2761    ; exp12
        DW      2767    ; expCommaExp
        DW      2776    ; fn1arg
        DW      2785    ; fn12arg
        DW      2797    ; fn2arg
        DW      2806    ; fn23arg
        DW      2818    ; fnBoundArg
        DW      2830    ; lbsExpComma
        DW      2845    ; lbsInpExpComma
        DW      2860    ; optCommaExp
        DW      2863    ; optFilenum
        DW      2875    ; parms
        DW      2892    ; parms1
        DW      2908    ; printItem
        DW      2960    ; printList
        DW      2978    ; printUsingItem


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

        DB      031H            , 01H           ; 4:  Exp->7
        DB      01H                             ; 6:  Reject
        DB      01bH            , 0FFH          ; 7:  optCommaExp->Accept
        DB      01H                             ; 9:  Reject

        DB      013H            , 01H           ; 10:  expCommaExp->13
        DB      01H                             ; 12:  Reject
        DB      09H             , 01H           ; 13:  commaExp->16
        DB      01H                             ; 15:  Reject
        DB      03H                             ; 16:  EMIT(opStBsave)
        DW      opStBsave                       
        DB      00H                             ; 19:  Accept

        DB      02H             , 01H           ; 20:  MARK(1)
        DB      044H            , 01H           ; 22:  IdSubRef->25
        DB      01H                             ; 24:  Reject
        DB      05fH            , 01H           ; 25:  tkLParen->28
        DB      00H                             ; 27:  Accept
        DB      03aH            , 01H           ; 28:  IdCallArg->31
        DB      01H                             ; 30:  Reject
        DB      063H            , 03H           ; 31:  tkComma->36
        DB      060H            , 0FFH          ; 33:  tkRParen->Accept
        DB      01H                             ; 35:  Reject
        DB      03aH            , 0d9H          ; 36:  IdCallArg->31
        DB      01H                             ; 38:  Reject

        DB      02H             , 01H           ; 39:  MARK(1)
        DB      044H            , 01H           ; 41:  IdSubRef->44
        DB      01H                             ; 43:  Reject
        DB      05fH            , 01H           ; 44:  tkLParen->47
        DB      00H                             ; 46:  Accept
        DB      031H            , 01H           ; 47:  Exp->50
        DB      01H                             ; 49:  Reject
        DB      063H            , 03H           ; 50:  tkComma->55
        DB      060H            , 0FFH          ; 52:  tkRParen->Accept
        DB      01H                             ; 54:  Reject
        DB      031H            , 0d9H          ; 55:  Exp->50
        DB      01H                             ; 57:  Reject

        DB      0a4H            , 09H           ; 58:  tkELSE->69
        DB      08H             , 01H           ; 60:  caseItem->63
        DB      01H                             ; 62:  Reject
        DB      063H            , 01H           ; 63:  tkComma->66
        DB      00H                             ; 65:  Accept
        DB      08H             , 0dbH          ; 66:  caseItem->63
        DB      01H                             ; 68:  Reject
        DB      03H                             ; 69:  EMIT(opStCaseElse)
        DW      opStCaseElse                    
        DB      00H                             ; 72:  Accept

        DB      031H            , 01H           ; 73:  Exp->76
        DB      01H                             ; 75:  Reject
        DB      03H                             ; 76:  EMIT(opStChain)
        DW      opStChain                       
        DB      00H                             ; 79:  Accept

        DB      031H            , 01H           ; 80:  Exp->83
        DB      01H                             ; 82:  Reject
        DB      03H                             ; 83:  EMIT(opStChdir)
        DW      opStChdir                       
        DB      00H                             ; 86:  Accept

        DB      0dH             , 01H           ; 87:  coordStep->90
        DB      01H                             ; 89:  Reject
        DB      09H             , 01H           ; 90:  commaExp->93
        DB      01H                             ; 92:  Reject
        DB      063H            , 01H           ; 93:  tkComma->96
        DB      00H                             ; 95:  Accept
        DB      031H            , 02H           ; 96:  Exp->100
        DB      04H             , 02H           ; 98:  empty->102
        DB      02H             , 01H           ; 100:  MARK(1)
        DB      027H            , 01H           ; 102:  CommaNoEos->105
        DB      00H                             ; 104:  Accept
        DB      031H            , 03H           ; 105:  Exp->110
        DB      03H                             ; 107:  EMIT(opNull)
        DW      opNull                          
        DB      03H                             ; 110:  EMIT(opCircleStart)
        DW      opCircleStart                   
        DB      027H            , 01H           ; 113:  CommaNoEos->116
        DB      00H                             ; 115:  Accept
        DB      031H            , 02H           ; 116:  Exp->120
        DB      04H             , 03H           ; 118:  empty->123
        DB      03H                             ; 120:  EMIT(opCircleEnd)
        DW      opCircleEnd                     
        DB      09H             , 01H           ; 123:  commaExp->126
        DB      00H                             ; 125:  Accept
        DB      03H                             ; 126:  EMIT(opCircleAspect)
        DW      opCircleAspect                  
        DB      00H                             ; 129:  Accept

        DB      04bH            , 0FFH          ; 130:  NArgsMax3->Accept
        DB      01H                             ; 132:  Reject

        DB      01cH            , 01H           ; 133:  optFilenum->136
        DB      00H                             ; 135:  Accept
        DB      063H            , 01H           ; 136:  tkComma->139
        DB      00H                             ; 138:  Accept
        DB      01cH            , 0dbH          ; 139:  optFilenum->136
        DB      01H                             ; 141:  Reject

        DB      031H            , 03H           ; 142:  Exp->147
        DB      03H                             ; 144:  EMIT(opUndef)
        DW      opUndef                         
        DB      03H                             ; 147:  EMIT(opStCls)
        DW      opStCls                         
        DB      00H                             ; 150:  Accept

        DB      04bH            , 0FFH          ; 151:  NArgsMax3->Accept
        DB      01H                             ; 153:  Reject

        DB      014H            , 01H           ; 154:  fn1arg->157
        DB      01H                             ; 156:  Reject
        DB      03H                             ; 157:  EMIT(opEvCom)
        DW      opEvCom                         
        DB      011H            , 0FFH          ; 160:  evSwitch->Accept
        DB      01H                             ; 162:  Reject

        DB      0e0H, 039H      , 011H          ; 163:  tkSHARED->183
        DB      03H                             ; 166:  EMIT(opStCommon)
        DW      opStCommon                      
        DB      0fH             , 01H           ; 169:  EMITFFFF->172
        DB      01H                             ; 171:  Reject
        DB      065H            , 03H           ; 172:  tkDiv->177
        DB      0fH             , 01eH          ; 174:  EMITFFFF->206
        DB      01H                             ; 176:  Reject
        DB      040H            , 01H           ; 177:  IdNamCom->180
        DB      01H                             ; 179:  Reject
        DB      065H            , 018H          ; 180:  tkDiv->206
        DB      01H                             ; 182:  Reject
        DB      03H                             ; 183:  EMIT(opShared)
        DW      opShared                        
        DB      03H                             ; 186:  EMIT(opStCommon)
        DW      opStCommon                      
        DB      0fH             , 01H           ; 189:  EMITFFFF->192
        DB      01H                             ; 191:  Reject
        DB      065H            , 03H           ; 192:  tkDiv->197
        DB      0fH             , 07H           ; 194:  EMITFFFF->203
        DB      01H                             ; 196:  Reject
        DB      040H            , 01H           ; 197:  IdNamCom->200
        DB      01H                             ; 199:  Reject
        DB      065H            , 01H           ; 200:  tkDiv->203
        DB      01H                             ; 202:  Reject
        DB      023H            , 01H           ; 203:  ACTIONidShared->206
        DB      01H                             ; 205:  Reject
        DB      022H            , 01H           ; 206:  ACTIONidCommon->209
        DB      01H                             ; 208:  Reject
        DB      038H            , 01H           ; 209:  IdAryI->212
        DB      01H                             ; 211:  Reject
        DB      063H            , 01H           ; 212:  tkComma->215
        DB      00H                             ; 214:  Accept
        DB      038H            , 0dbH          ; 215:  IdAryI->212
        DB      01H                             ; 217:  Reject

        DB      03H                             ; 218:  EMIT(opStConst)
        DW      opStConst                       
        DB      028H            , 01H           ; 221:  ConstAssign->224
        DB      01H                             ; 223:  Reject
        DB      063H            , 01H           ; 224:  tkComma->227
        DB      00H                             ; 226:  Accept
        DB      028H            , 0dbH          ; 227:  ConstAssign->224
        DB      01H                             ; 229:  Reject

        DB      069H            , 01H           ; 230:  tkEQ->233
        DB      01H                             ; 232:  Reject
        DB      031H            , 01H           ; 233:  Exp->236
        DB      01H                             ; 235:  Reject
        DB      03H                             ; 236:  EMIT(opStDate_)
        DW      opStDate_                       
        DB      00H                             ; 239:  Accept

        DB      0bbH            , 07H           ; 240:  tkFUNCTION->249
        DB      0e0H, 04bH      , 01H           ; 242:  tkSUB->246
        DB      01H                             ; 245:  Reject
        DB      042H            , 04H           ; 246:  IdSubDecl->252
        DB      01H                             ; 248:  Reject
        DB      03dH            , 01H           ; 249:  IdFuncDecl->252
        DB      01H                             ; 251:  Reject
        DB      02H             , 03H           ; 252:  MARK(3)
        DB      01dH            , 0FFH          ; 254:  parms->Accept
        DB      01H                             ; 256:  Reject

        DB      03cH            , 01H           ; 257:  IdFn->260
        DB      01H                             ; 259:  Reject
        DB      02H             , 03H           ; 260:  MARK(3)
        DB      01dH            , 01H           ; 262:  parms->265
        DB      01H                             ; 264:  Reject
        DB      069H            , 01H           ; 265:  tkEQ->268
        DB      00H                             ; 267:  Accept
        DB      02H             , 05H           ; 268:  MARK(5)
        DB      031H            , 0FFH          ; 270:  Exp->Accept
        DB      01H                             ; 272:  Reject

        DB      0e0H, 035H      , 01H           ; 273:  tkSEG->277
        DB      01H                             ; 276:  Reject
        DB      069H            , 01H           ; 277:  tkEQ->280
        DB      00H                             ; 279:  Accept
        DB      031H            , 0FFH          ; 280:  Exp->Accept
        DB      01H                             ; 282:  Reject

        DB      029H            , 0FFH          ; 283:  DeflistI2->Accept
        DB      01H                             ; 285:  Reject

        DB      02aH            , 0FFH          ; 286:  DeflistI4->Accept
        DB      01H                             ; 288:  Reject

        DB      02bH            , 0FFH          ; 289:  DeflistR4->Accept
        DB      01H                             ; 291:  Reject

        DB      02cH            , 0FFH          ; 292:  DeflistR8->Accept
        DB      01H                             ; 294:  Reject

        DB      02dH            , 0FFH          ; 295:  DeflistSD->Accept
        DB      01H                             ; 297:  Reject

        DB      0e0H, 039H      , 02H           ; 298:  tkSHARED->303
        DB      04H             , 06H           ; 301:  empty->309
        DB      023H            , 01H           ; 303:  ACTIONidShared->306
        DB      01H                             ; 305:  Reject
        DB      03H                             ; 306:  EMIT(opShared)
        DW      opShared                        
        DB      03H                             ; 309:  EMIT(opStDim)
        DW      opStDim                         
        DB      0fH             , 01H           ; 312:  EMITFFFF->315
        DB      01H                             ; 314:  Reject
        DB      033H            , 01H           ; 315:  IdAryDim->318
        DB      01H                             ; 317:  Reject
        DB      063H            , 01H           ; 318:  tkComma->321
        DB      00H                             ; 320:  Accept
        DB      033H            , 0dbH          ; 321:  IdAryDim->318
        DB      01H                             ; 323:  Reject

        DB      0e0H, 064H      , 010H          ; 324:  tkWHILE->343
        DB      0e0H, 05bH      , 04H           ; 327:  tkUNTIL->334
        DB      03H                             ; 330:  EMIT(opStDo)
        DW      opStDo                          
        DB      00H                             ; 333:  Accept
        DB      031H            , 01H           ; 334:  Exp->337
        DB      01H                             ; 336:  Reject
        DB      03H                             ; 337:  EMIT(opStDoUntil)
        DW      opStDoUntil                     
        DB      0fH             , 0FFH          ; 340:  EMITFFFF->Accept
        DB      01H                             ; 342:  Reject
        DB      031H            , 01H           ; 343:  Exp->346
        DB      01H                             ; 345:  Reject
        DB      03H                             ; 346:  EMIT(opStDoWhile)
        DW      opStDoWhile                     
        DB      0fH             , 0FFH          ; 349:  EMITFFFF->Accept
        DB      01H                             ; 351:  Reject

        DB      031H            , 01H           ; 352:  Exp->355
        DB      01H                             ; 354:  Reject
        DB      03H                             ; 355:  EMIT(opStDraw)
        DW      opStDraw                        
        DB      00H                             ; 358:  Accept

        DB      031H            , 01H           ; 359:  Exp->362
        DB      01H                             ; 361:  Reject
        DB      0e0H, 050H      , 01H           ; 362:  tkTHEN->366
        DB      01H                             ; 365:  Reject
        DB      03H                             ; 366:  EMIT(opStElseIf)
        DW      opStElseIf                      
        DB      0fH             , 01H           ; 369:  EMITFFFF->372
        DB      01H                             ; 371:  Reject
        DB      052H            , 0FFH          ; 372:  StatementList->Accept
        DB      00H                             ; 374:  Accept

        DB      03H                             ; 375:  EMIT(opStElse)
        DW      opStElse                        
        DB      0fH             , 01H           ; 378:  EMITFFFF->381
        DB      01H                             ; 380:  Reject
        DB      052H            , 0FFH          ; 381:  StatementList->Accept
        DB      00H                             ; 383:  Accept

        DB      09aH            , 029H          ; 384:  tkDEF->427
        DB      0bbH            , 023H          ; 386:  tkFUNCTION->423
        DB      0c0H            , 01bH          ; 388:  tkIF->417
        DB      0e0H, 036H      , 014H          ; 390:  tkSELECT->413
        DB      0e0H, 04bH      , 0dH           ; 393:  tkSUB->409
        DB      0e0H, 056H      , 04H           ; 396:  tkTYPE->403
        DB      03H                             ; 399:  EMIT(opStEnd)
        DW      opStEnd                         
        DB      00H                             ; 402:  Accept
        DB      03H                             ; 403:  EMIT(opStEndType)
        DW      opStEndType                     
        DB      0fH             , 0FFH          ; 406:  EMITFFFF->Accept
        DB      01H                             ; 408:  Reject
        DB      03H                             ; 409:  EMIT(opStEndProc)
        DW      opStEndProc                     
        DB      00H                             ; 412:  Accept
        DB      03H                             ; 413:  EMIT(opStEndSelect)
        DW      opStEndSelect                   
        DB      00H                             ; 416:  Accept
        DB      03H                             ; 417:  EMIT(opStEndIfBlock)
        DW      opStEndIfBlock                  
        DB      030H            , 0FFH          ; 420:  ErrIfNot1st->Accept
        DB      01H                             ; 422:  Reject
        DB      03H                             ; 423:  EMIT(opStEndProc)
        DW      opStEndProc                     
        DB      00H                             ; 426:  Accept
        DB      03H                             ; 427:  EMIT(opStEndDef)
        DW      opStEndDef                      
        DB      03H                             ; 430:  EMIT(2)
        DW      2
        DB      0fH             , 0FFH          ; 433:  EMITFFFF->Accept
        DB      01H                             ; 435:  Reject

        DB      03H                             ; 436:  EMIT(opStEndIfBlock)
        DW      opStEndIfBlock                  
        DB      030H            , 0FFH          ; 439:  ErrIfNot1st->Accept
        DB      01H                             ; 441:  Reject

        DB      031H            , 01H           ; 442:  Exp->445
        DB      01H                             ; 444:  Reject
        DB      03H                             ; 445:  EMIT(opStEnviron)
        DW      opStEnviron                     
        DB      00H                             ; 448:  Accept

        DB      039H            , 01H           ; 449:  IdArray->452
        DB      01H                             ; 451:  Reject
        DB      063H            , 01H           ; 452:  tkComma->455
        DB      00H                             ; 454:  Accept
        DB      039H            , 0dbH          ; 455:  IdArray->452
        DB      01H                             ; 457:  Reject

        DB      031H            , 01H           ; 458:  Exp->461
        DB      01H                             ; 460:  Reject
        DB      03H                             ; 461:  EMIT(opStError)
        DW      opStError                       
        DB      00H                             ; 464:  Accept

        DB      09aH            , 014H          ; 465:  tkDEF->487
        DB      0bbH            , 012H          ; 467:  tkFUNCTION->487
        DB      0e0H, 04bH      , 0fH           ; 469:  tkSUB->487
        DB      0a1H            , 08H           ; 472:  tkDO->482
        DB      0b8H            , 01H           ; 474:  tkFOR->477
        DB      01H                             ; 476:  Reject
        DB      03H                             ; 477:  EMIT(opStExitFor)
        DW      opStExitFor                     
        DB      04H             , 08H           ; 480:  empty->490
        DB      03H                             ; 482:  EMIT(opStExitDo)
        DW      opStExitDo                      
        DB      04H             , 03H           ; 485:  empty->490
        DB      03H                             ; 487:  EMIT(opStExitProc)
        DW      opStExitProc                    
        DB      0fH             , 0FFH          ; 490:  EMITFFFF->Accept
        DB      01H                             ; 492:  Reject

        DB      01cH            , 01H           ; 493:  optFilenum->496
        DB      01H                             ; 495:  Reject
        DB      03H                             ; 496:  EMIT(opFieldInit)
        DW      opFieldInit                     
        DB      09H             , 01H           ; 499:  commaExp->502
        DB      01H                             ; 501:  Reject
        DB      072H            , 01H           ; 502:  tkAS->505
        DB      01H                             ; 504:  Reject
        DB      035H            , 01H           ; 505:  IdAryElem->508
        DB      01H                             ; 507:  Reject
        DB      03H                             ; 508:  EMIT(opFieldItem)
        DW      opFieldItem                     
        DB      09H             , 01H           ; 511:  commaExp->514
        DB      00H                             ; 513:  Accept
        DB      072H            , 01H           ; 514:  tkAS->517
        DB      01H                             ; 516:  Reject
        DB      035H            , 01H           ; 517:  IdAryElem->520
        DB      01H                             ; 519:  Reject
        DB      03H                             ; 520:  EMIT(opFieldItem)
        DW      opFieldItem                     
        DB      04H             , 0d2H          ; 523:  empty->511

        DB      031H            , 0FFH          ; 525:  Exp->Accept
        DB      00H                             ; 527:  Accept

        DB      03bH            , 01H           ; 528:  IdFor->531
        DB      01H                             ; 530:  Reject
        DB      069H            , 01H           ; 531:  tkEQ->534
        DB      01H                             ; 533:  Reject
        DB      031H            , 01H           ; 534:  Exp->537
        DB      01H                             ; 536:  Reject
        DB      0e0H, 053H      , 01H           ; 537:  tkTO->541
        DB      01H                             ; 540:  Reject
        DB      031H            , 01H           ; 541:  Exp->544
        DB      01H                             ; 543:  Reject
        DB      0e0H, 044H      , 05H           ; 544:  tkSTEP->552
        DB      03H                             ; 547:  EMIT(opStFor)
        DW      opStFor                         
        DB      04H             , 06H           ; 550:  empty->558
        DB      031H            , 01H           ; 552:  Exp->555
        DB      01H                             ; 554:  Reject
        DB      03H                             ; 555:  EMIT(opStForStep)
        DW      opStForStep                     
        DB      0fH             , 01H           ; 558:  EMITFFFF->561
        DB      01H                             ; 560:  Reject
        DB      0fH             , 0FFH          ; 561:  EMITFFFF->Accept
        DB      01H                             ; 563:  Reject

        DB      030H            , 01H           ; 564:  ErrIfNot1st->567
        DB      01H                             ; 566:  Reject
        DB      03eH            , 01H           ; 567:  IdFuncDef->570
        DB      01H                             ; 569:  Reject
        DB      02H             , 03H           ; 570:  MARK(3)
        DB      01dH            , 01H           ; 572:  parms->575
        DB      01H                             ; 574:  Reject
        DB      0e0H, 043H      , 01H           ; 575:  tkSTATIC->579
        DB      00H                             ; 578:  Accept
        DB      02H             , 04H           ; 579:  MARK(4)
        DB      00H                             ; 581:  Accept

        DB      01cH            , 01H           ; 582:  optFilenum->585
        DB      01H                             ; 584:  Reject
        DB      063H            , 04H           ; 585:  tkComma->591
        DB      03H                             ; 587:  EMIT(opStGet1)
        DW      opStGet1                        
        DB      00H                             ; 590:  Accept
        DB      031H            , 0cH           ; 591:  Exp->605
        DB      063H            , 01H           ; 593:  tkComma->596
        DB      01H                             ; 595:  Reject
        DB      036H            , 01H           ; 596:  IdAryElemRef->599
        DB      01H                             ; 598:  Reject
        DB      03H                             ; 599:  EMIT(opStGetRec2)
        DW      opStGetRec2                     
        DB      0fH             , 0FFH          ; 602:  EMITFFFF->Accept
        DB      01H                             ; 604:  Reject
        DB      063H            , 04H           ; 605:  tkComma->611
        DB      03H                             ; 607:  EMIT(opStGet2)
        DW      opStGet2                        
        DB      00H                             ; 610:  Accept
        DB      036H            , 01H           ; 611:  IdAryElemRef->614
        DB      01H                             ; 613:  Reject
        DB      03H                             ; 614:  EMIT(opStGetRec3)
        DW      opStGetRec3                     
        DB      0fH             , 0FFH          ; 617:  EMITFFFF->Accept
        DB      01H                             ; 619:  Reject

        DB      0dH             , 01H           ; 620:  coordStep->623
        DB      01H                             ; 622:  Reject
        DB      064H            , 01H           ; 623:  tkMinus->626
        DB      01H                             ; 625:  Reject
        DB      0eH             , 01H           ; 626:  coord2Step->629
        DB      01H                             ; 628:  Reject
        DB      063H            , 01H           ; 629:  tkComma->632
        DB      01H                             ; 631:  Reject
        DB      037H            , 01H           ; 632:  IdAryGetPut->635
        DB      01H                             ; 634:  Reject
        DB      03H                             ; 635:  EMIT(opStGraphicsGet)
        DW      opStGraphicsGet                 
        DB      00H                             ; 638:  Accept

        DB      03H                             ; 639:  EMIT(opStGosub)
        DW      opStGosub                       
        DB      046H            , 0FFH          ; 642:  LabLn->Accept
        DB      01H                             ; 644:  Reject

        DB      03H                             ; 645:  EMIT(opStGoto)
        DW      opStGoto                        
        DB      046H            , 0FFH          ; 648:  LabLn->Accept
        DB      01H                             ; 650:  Reject

        DB      031H            , 01H           ; 651:  Exp->654
        DB      01H                             ; 653:  Reject
        DB      045H            , 0FFH          ; 654:  IfStmt->Accept
        DB      01H                             ; 656:  Reject

        DB      01aH            , 022H          ; 657:  lbsInpExpComma->693
        DB      067H            , 0fH           ; 659:  tkSColon->676
        DB      047H            , 02H           ; 661:  LitString->665
        DB      04H             , 01eH          ; 663:  empty->695
        DB      02H             , 04H           ; 665:  MARK(4)
        DB      067H            , 01aH          ; 667:  tkSColon->695
        DB      063H            , 01H           ; 669:  tkComma->672
        DB      01H                             ; 671:  Reject
        DB      02H             , 01H           ; 672:  MARK(1)
        DB      04H             , 013H          ; 674:  empty->695
        DB      02H             , 02H           ; 676:  MARK(2)
        DB      047H            , 02H           ; 678:  LitString->682
        DB      04H             , 0dH           ; 680:  empty->695
        DB      02H             , 04H           ; 682:  MARK(4)
        DB      067H            , 09H           ; 684:  tkSColon->695
        DB      063H            , 01H           ; 686:  tkComma->689
        DB      01H                             ; 688:  Reject
        DB      02H             , 01H           ; 689:  MARK(1)
        DB      04H             , 02H           ; 691:  empty->695
        DB      02H             , 010H          ; 693:  MARK(16)
        DB      02H             , 08H           ; 695:  MARK(8)
        DB      036H            , 01H           ; 697:  IdAryElemRef->700
        DB      01H                             ; 699:  Reject
        DB      03H                             ; 700:  EMIT(opStInput)
        DW      opStInput                       
        DB      063H            , 04H           ; 703:  tkComma->709
        DB      03H                             ; 705:  EMIT(opInputEos)
        DW      opInputEos                      
        DB      00H                             ; 708:  Accept
        DB      036H            , 01H           ; 709:  IdAryElemRef->712
        DB      01H                             ; 711:  Reject
        DB      03H                             ; 712:  EMIT(opStInput)
        DW      opStInput                       
        DB      04H             , 0d2H          ; 715:  empty->703

        DB      01cH            , 01H           ; 717:  optFilenum->720
        DB      01H                             ; 719:  Reject
        DB      09H             , 01H           ; 720:  commaExp->723
        DB      01H                             ; 722:  Reject
        DB      03H                             ; 723:  EMIT(opStIoctl)
        DW      opStIoctl                       
        DB      00H                             ; 726:  Accept

        DB      0e0H, 0eH       , 022H          ; 727:  tkOFF->764
        DB      0e0H, 0fH       , 018H          ; 730:  tkON->757
        DB      0d4H            , 0fH           ; 733:  tkLIST->750
        DB      014H            , 07H           ; 735:  fn1arg->744
        DB      013H            , 01H           ; 737:  expCommaExp->740
        DB      01H                             ; 739:  Reject
        DB      03H                             ; 740:  EMIT(opStKeyMap)
        DW      opStKeyMap                      
        DB      00H                             ; 743:  Accept
        DB      03H                             ; 744:  EMIT(opEvKey)
        DW      opEvKey                         
        DB      011H            , 0FFH          ; 747:  evSwitch->Accept
        DB      01H                             ; 749:  Reject
        DB      03H                             ; 750:  EMIT(opStKey)
        DW      opStKey                         
        DB      03H                             ; 753:  EMIT(2)
        DW      2
        DB      00H                             ; 756:  Accept
        DB      03H                             ; 757:  EMIT(opStKey)
        DW      opStKey                         
        DB      03H                             ; 760:  EMIT(1)
        DW      1
        DB      00H                             ; 763:  Accept
        DB      03H                             ; 764:  EMIT(opStKey)
        DW      opStKey                         
        DB      03H                             ; 767:  EMIT(0)
        DW      0
        DB      00H                             ; 770:  Accept

        DB      031H            , 01H           ; 771:  Exp->774
        DB      01H                             ; 773:  Reject
        DB      03H                             ; 774:  EMIT(opStKill)
        DW      opStKill                        
        DB      00H                             ; 777:  Accept

        DB      03H                             ; 778:  EMIT(opStLet)
        DW      opStLet                         
        DB      025H            , 0FFH          ; 781:  Assignment->Accept
        DB      01H                             ; 783:  Reject

        DB      0dH             , 00H           ; 784:  coordStep->786
        DB      064H            , 01H           ; 786:  tkMinus->789
        DB      01H                             ; 788:  Reject
        DB      0eH             , 01H           ; 789:  coord2Step->792
        DB      01H                             ; 791:  Reject
        DB      063H            , 01H           ; 792:  tkComma->795
        DB      00H                             ; 794:  Accept
        DB      031H            , 02H           ; 795:  Exp->799
        DB      04H             , 02H           ; 797:  empty->801
        DB      02H             , 01H           ; 799:  MARK(1)
        DB      063H            , 01H           ; 801:  tkComma->804
        DB      00H                             ; 803:  Accept
        DB      050H            , 0eH           ; 804:  RwBF->820
        DB      04eH            , 02H           ; 806:  RwB->810
        DB      04H             , 0cH           ; 808:  empty->822
        DB      04fH            , 04H           ; 810:  RwF->816
        DB      02H             , 02H           ; 812:  MARK(2)
        DB      04H             , 06H           ; 814:  empty->822
        DB      02H             , 03H           ; 816:  MARK(3)
        DB      04H             , 02H           ; 818:  empty->822
        DB      02H             , 03H           ; 820:  MARK(3)
        DB      09H             , 01H           ; 822:  commaExp->825
        DB      00H                             ; 824:  Accept
        DB      02H             , 04H           ; 825:  MARK(4)
        DB      00H                             ; 827:  Accept

        DB      0c4H            , 01H           ; 828:  tkINPUT->831
        DB      01H                             ; 830:  Reject
        DB      01aH            , 022H          ; 831:  lbsInpExpComma->867
        DB      067H            , 0fH           ; 833:  tkSColon->850
        DB      047H            , 02H           ; 835:  LitString->839
        DB      04H             , 01eH          ; 837:  empty->869
        DB      02H             , 04H           ; 839:  MARK(4)
        DB      067H            , 01aH          ; 841:  tkSColon->869
        DB      063H            , 01H           ; 843:  tkComma->846
        DB      01H                             ; 845:  Reject
        DB      02H             , 01H           ; 846:  MARK(1)
        DB      04H             , 013H          ; 848:  empty->869
        DB      02H             , 02H           ; 850:  MARK(2)
        DB      047H            , 02H           ; 852:  LitString->856
        DB      04H             , 0dH           ; 854:  empty->869
        DB      02H             , 04H           ; 856:  MARK(4)
        DB      067H            , 09H           ; 858:  tkSColon->869
        DB      063H            , 01H           ; 860:  tkComma->863
        DB      01H                             ; 862:  Reject
        DB      02H             , 01H           ; 863:  MARK(1)
        DB      04H             , 02H           ; 865:  empty->869
        DB      02H             , 010H          ; 867:  MARK(16)
        DB      036H            , 0FFH          ; 869:  IdAryElemRef->Accept
        DB      01H                             ; 871:  Reject

        DB      04dH            , 0FFH          ; 872:  NArgsMax5->Accept
        DB      01H                             ; 874:  Reject

        DB      01cH            , 01H           ; 875:  optFilenum->878
        DB      01H                             ; 877:  Reject
        DB      063H            , 01H           ; 878:  tkComma->881
        DB      00H                             ; 880:  Accept
        DB      031H            , 09H           ; 881:  Exp->892
        DB      0e0H, 053H      , 01H           ; 883:  tkTO->887
        DB      01H                             ; 886:  Reject
        DB      02H             , 03H           ; 887:  MARK(3)
        DB      031H            , 0FFH          ; 889:  Exp->Accept
        DB      01H                             ; 891:  Reject
        DB      02H             , 01H           ; 892:  MARK(1)
        DB      0e0H, 053H      , 01H           ; 894:  tkTO->898
        DB      00H                             ; 897:  Accept
        DB      02H             , 02H           ; 898:  MARK(2)
        DB      031H            , 0FFH          ; 900:  Exp->Accept
        DB      01H                             ; 902:  Reject

        DB      0e0H, 064H      , 010H          ; 903:  tkWHILE->922
        DB      0e0H, 05bH      , 05H           ; 906:  tkUNTIL->914
        DB      03H                             ; 909:  EMIT(opStLoop)
        DW      opStLoop                        
        DB      04H             , 0eH           ; 912:  empty->928
        DB      031H            , 01H           ; 914:  Exp->917
        DB      01H                             ; 916:  Reject
        DB      03H                             ; 917:  EMIT(opStLoopUntil)
        DW      opStLoopUntil                   
        DB      04H             , 06H           ; 920:  empty->928
        DB      031H            , 01H           ; 922:  Exp->925
        DB      01H                             ; 924:  Reject
        DB      03H                             ; 925:  EMIT(opStLoopWhile)
        DW      opStLoopWhile                   
        DB      0fH             , 0FFH          ; 928:  EMITFFFF->Accept
        DB      01H                             ; 930:  Reject

        DB      03H                             ; 931:  EMIT(opStLprint)
        DW      opStLprint                      
        DB      020H            , 0FFH          ; 934:  printList->Accept
        DB      01H                             ; 936:  Reject

        DB      02H             , 01H           ; 937:  MARK(1)
        DB      036H            , 01H           ; 939:  IdAryElemRef->942
        DB      01H                             ; 941:  Reject
        DB      02H             , 02H           ; 942:  MARK(2)
        DB      069H            , 01H           ; 944:  tkEQ->947
        DB      01H                             ; 946:  Reject
        DB      031H            , 0FFH          ; 947:  Exp->Accept
        DB      01H                             ; 949:  Reject

        DB      05fH            , 01H           ; 950:  tkLParen->953
        DB      01H                             ; 952:  Reject
        DB      02H             , 01H           ; 953:  MARK(1)
        DB      036H            , 01H           ; 955:  IdAryElemRef->958
        DB      01H                             ; 957:  Reject
        DB      02H             , 02H           ; 958:  MARK(2)
        DB      012H            , 01H           ; 960:  exp12->963
        DB      01H                             ; 962:  Reject
        DB      060H            , 01H           ; 963:  tkRParen->966
        DB      01H                             ; 965:  Reject
        DB      069H            , 01H           ; 966:  tkEQ->969
        DB      01H                             ; 968:  Reject
        DB      031H            , 0FFH          ; 969:  Exp->Accept
        DB      01H                             ; 971:  Reject

        DB      031H            , 01H           ; 972:  Exp->975
        DB      01H                             ; 974:  Reject
        DB      03H                             ; 975:  EMIT(opStMkdir)
        DW      opStMkdir                       
        DB      00H                             ; 978:  Accept

        DB      031H            , 01H           ; 979:  Exp->982
        DB      01H                             ; 981:  Reject
        DB      072H            , 01H           ; 982:  tkAS->985
        DB      01H                             ; 984:  Reject
        DB      031H            , 01H           ; 985:  Exp->988
        DB      01H                             ; 987:  Reject
        DB      03H                             ; 988:  EMIT(opStName)
        DW      opStName                        
        DB      00H                             ; 991:  Accept

        DB      03bH            , 09H           ; 992:  IdFor->1003
        DB      03H                             ; 994:  EMIT(opStNext)
        DW      opStNext                        
        DB      0fH             , 01H           ; 997:  EMITFFFF->1000
        DB      01H                             ; 999:  Reject
        DB      0fH             , 0FFH          ; 1000:  EMITFFFF->Accept
        DB      01H                             ; 1002:  Reject
        DB      03H                             ; 1003:  EMIT(opStNextId)
        DW      opStNextId                      
        DB      0fH             , 01H           ; 1006:  EMITFFFF->1009
        DB      01H                             ; 1008:  Reject
        DB      0fH             , 01H           ; 1009:  EMITFFFF->1012
        DB      01H                             ; 1011:  Reject
        DB      063H            , 01H           ; 1012:  tkComma->1015
        DB      00H                             ; 1014:  Accept
        DB      03bH            , 01H           ; 1015:  IdFor->1018
        DB      01H                             ; 1017:  Reject
        DB      03H                             ; 1018:  EMIT(opStNextId)
        DW      opStNextId                      
        DB      0fH             , 01H           ; 1021:  EMITFFFF->1024
        DB      01H                             ; 1023:  Reject
        DB      0fH             , 0d2H          ; 1024:  EMITFFFF->1012
        DB      01H                             ; 1026:  Reject

        DB      010H            , 02aH          ; 1027:  event->1071
        DB      0b1H            , 017H          ; 1029:  tkERROR->1054
        DB      031H            , 01H           ; 1031:  Exp->1034
        DB      01H                             ; 1033:  Reject
        DB      0beH            , 07H           ; 1034:  tkGOTO->1043
        DB      0bdH            , 01H           ; 1036:  tkGOSUB->1039
        DB      01H                             ; 1038:  Reject
        DB      02H             , 02H           ; 1039:  MARK(2)
        DB      04H             , 02H           ; 1041:  empty->1045
        DB      02H             , 01H           ; 1043:  MARK(1)
        DB      046H            , 01H           ; 1045:  LabLn->1048
        DB      01H                             ; 1047:  Reject
        DB      063H            , 01H           ; 1048:  tkComma->1051
        DB      00H                             ; 1050:  Accept
        DB      046H            , 0dbH          ; 1051:  LabLn->1048
        DB      01H                             ; 1053:  Reject
        DB      0beH            , 01H           ; 1054:  tkGOTO->1057
        DB      01H                             ; 1056:  Reject
        DB      048H            , 06H           ; 1057:  Lit0->1065
        DB      03H                             ; 1059:  EMIT(opStOnError)
        DW      opStOnError                     
        DB      046H            , 0FFH          ; 1062:  LabLn->Accept
        DB      01H                             ; 1064:  Reject
        DB      03H                             ; 1065:  EMIT(opStOnError)
        DW      opStOnError                     
        DB      0fH             , 0FFH          ; 1068:  EMITFFFF->Accept
        DB      01H                             ; 1070:  Reject
        DB      0bdH            , 01H           ; 1071:  tkGOSUB->1074
        DB      01H                             ; 1073:  Reject
        DB      048H            , 06H           ; 1074:  Lit0->1082
        DB      03H                             ; 1076:  EMIT(opEvGosub)
        DW      opEvGosub                       
        DB      046H            , 0FFH          ; 1079:  LabLn->Accept
        DB      01H                             ; 1081:  Reject
        DB      03H                             ; 1082:  EMIT(opEvGosub)
        DW      opEvGosub                       
        DB      0fH             , 0FFH          ; 1085:  EMITFFFF->Accept
        DB      01H                             ; 1087:  Reject

        DB      031H            , 01H           ; 1088:  Exp->1091
        DB      01H                             ; 1090:  Reject
        DB      0b8H            , 02H           ; 1091:  tkFOR->1095
        DB      04H             , 01fH          ; 1093:  empty->1126
        DB      071H            , 01bH          ; 1095:  tkAPPEND->1124
        DB      0c4H            , 015H          ; 1097:  tkINPUT->1120
        DB      0e0H, 014H      , 0eH           ; 1099:  tkOUTPUT->1116
        DB      0e0H, 023H      , 07H           ; 1102:  tkRANDOM->1112
        DB      077H            , 01H           ; 1105:  tkBINARY->1108
        DB      01H                             ; 1107:  Reject
        DB      02H             , 05H           ; 1108:  MARK(5)
        DB      04H             , 0eH           ; 1110:  empty->1126
        DB      02H             , 04H           ; 1112:  MARK(4)
        DB      04H             , 0aH           ; 1114:  empty->1126
        DB      02H             , 03H           ; 1116:  MARK(3)
        DB      04H             , 06H           ; 1118:  empty->1126
        DB      02H             , 02H           ; 1120:  MARK(2)
        DB      04H             , 02H           ; 1122:  empty->1126
        DB      02H             , 01H           ; 1124:  MARK(1)
        DB      06dH            , 02H           ; 1126:  tkACCESS->1130
        DB      04H             , 014H          ; 1128:  empty->1150
        DB      0e0H, 025H      , 08H           ; 1130:  tkREAD->1141
        DB      0e0H, 067H      , 01H           ; 1133:  tkWRITE->1137
        DB      01H                             ; 1136:  Reject
        DB      02H             , 07H           ; 1137:  MARK(7)
        DB      04H             , 09H           ; 1139:  empty->1150
        DB      02H             , 06H           ; 1141:  MARK(6)
        DB      0e0H, 067H      , 02H           ; 1143:  tkWRITE->1148
        DB      04H             , 02H           ; 1146:  empty->1150
        DB      02H             , 08H           ; 1148:  MARK(8)
        DB      0d8H            , 09H           ; 1150:  tkLOCK->1161
        DB      0e0H, 039H      , 02H           ; 1152:  tkSHARED->1157
        DB      04H             , 018H          ; 1155:  empty->1181
        DB      02H             , 0cH           ; 1157:  MARK(12)
        DB      04H             , 014H          ; 1159:  empty->1181
        DB      0e0H, 025H      , 08H           ; 1161:  tkREAD->1172
        DB      0e0H, 067H      , 01H           ; 1164:  tkWRITE->1168
        DB      01H                             ; 1167:  Reject
        DB      02H             , 0aH           ; 1168:  MARK(10)
        DB      04H             , 09H           ; 1170:  empty->1181
        DB      0e0H, 067H      , 04H           ; 1172:  tkWRITE->1179
        DB      02H             , 09H           ; 1175:  MARK(9)
        DB      04H             , 02H           ; 1177:  empty->1181
        DB      02H             , 0bH           ; 1179:  MARK(11)
        DB      072H            , 0cH           ; 1181:  tkAS->1195
        DB      063H            , 01H           ; 1183:  tkComma->1186
        DB      01H                             ; 1185:  Reject
        DB      01cH            , 01H           ; 1186:  optFilenum->1189
        DB      01H                             ; 1188:  Reject
        DB      012H            , 01H           ; 1189:  exp12->1192
        DB      01H                             ; 1191:  Reject
        DB      02H             , 0eH           ; 1192:  MARK(14)
        DB      00H                             ; 1194:  Accept
        DB      01cH            , 01H           ; 1195:  optFilenum->1198
        DB      01H                             ; 1197:  Reject
        DB      0d1H            , 01H           ; 1198:  tkLEN->1201
        DB      00H                             ; 1200:  Accept
        DB      069H            , 01H           ; 1201:  tkEQ->1204
        DB      01H                             ; 1203:  Reject
        DB      031H            , 01H           ; 1204:  Exp->1207
        DB      01H                             ; 1206:  Reject
        DB      02H             , 0dH           ; 1207:  MARK(13)
        DB      00H                             ; 1209:  Accept

        DB      075H            , 01H           ; 1210:  tkBASE->1213
        DB      01H                             ; 1212:  Reject
        DB      048H            , 07H           ; 1213:  Lit0->1222
        DB      049H            , 01H           ; 1215:  Lit1->1218
        DB      01H                             ; 1217:  Reject
        DB      03H                             ; 1218:  EMIT(opStOptionBase1)
        DW      opStOptionBase1                 
        DB      00H                             ; 1221:  Accept
        DB      03H                             ; 1222:  EMIT(opStOptionBase0)
        DW      opStOptionBase0                 
        DB      00H                             ; 1225:  Accept

        DB      013H            , 01H           ; 1226:  expCommaExp->1229
        DB      01H                             ; 1228:  Reject
        DB      03H                             ; 1229:  EMIT(opStOut)
        DW      opStOut                         
        DB      00H                             ; 1232:  Accept

        DB      0dH             , 01H           ; 1233:  coordStep->1236
        DB      01H                             ; 1235:  Reject
        DB      0aH             , 01H           ; 1236:  commaOptExp->1239
        DB      01H                             ; 1238:  Reject
        DB      0aH             , 01H           ; 1239:  commaOptExp->1242
        DB      01H                             ; 1241:  Reject
        DB      09H             , 04H           ; 1242:  commaExp->1248
        DB      03H                             ; 1244:  EMIT(opStPaint2)
        DW      opStPaint2                      
        DB      00H                             ; 1247:  Accept
        DB      03H                             ; 1248:  EMIT(opStPaint3)
        DW      opStPaint3                      
        DB      00H                             ; 1251:  Accept

        DB      0e0H, 05cH      , 0aH           ; 1252:  tkUSING->1265
        DB      013H            , 04H           ; 1255:  expCommaExp->1261
        DB      03H                             ; 1257:  EMIT(opStPalette0)
        DW      opStPalette0                    
        DB      00H                             ; 1260:  Accept
        DB      03H                             ; 1261:  EMIT(opStPalette2)
        DW      opStPalette2                    
        DB      00H                             ; 1264:  Accept
        DB      037H            , 01H           ; 1265:  IdAryGetPut->1268
        DB      01H                             ; 1267:  Reject
        DB      03H                             ; 1268:  EMIT(opStPaletteUsing)
        DW      opStPaletteUsing                
        DB      00H                             ; 1271:  Accept

        DB      013H            , 01H           ; 1272:  expCommaExp->1275
        DB      01H                             ; 1274:  Reject
        DB      03H                             ; 1275:  EMIT(opStPCopy)
        DW      opStPCopy                       
        DB      00H                             ; 1278:  Accept

        DB      03H                             ; 1279:  EMIT(opEvPen)
        DW      opEvPen                         
        DB      011H            , 0FFH          ; 1282:  evSwitch->Accept
        DB      01H                             ; 1284:  Reject

        DB      031H            , 01H           ; 1285:  Exp->1288
        DB      01H                             ; 1287:  Reject
        DB      03H                             ; 1288:  EMIT(opStPlay)
        DW      opStPlay                        
        DB      00H                             ; 1291:  Accept

        DB      03H                             ; 1292:  EMIT(opEvPlay0)
        DW      opEvPlay0                       
        DB      011H            , 0FFH          ; 1295:  evSwitch->Accept
        DB      01H                             ; 1297:  Reject

        DB      013H            , 01H           ; 1298:  expCommaExp->1301
        DB      01H                             ; 1300:  Reject
        DB      03H                             ; 1301:  EMIT(opStPoke)
        DW      opStPoke                        
        DB      00H                             ; 1304:  Accept

        DB      0dH             , 01H           ; 1305:  coordStep->1308
        DB      01H                             ; 1307:  Reject
        DB      01bH            , 0FFH          ; 1308:  optCommaExp->Accept
        DB      01H                             ; 1310:  Reject

        DB      019H            , 00H           ; 1311:  lbsExpComma->1313
        DB      020H            , 0FFH          ; 1313:  printList->Accept
        DB      01H                             ; 1315:  Reject

        DB      0dH             , 01H           ; 1316:  coordStep->1319
        DB      01H                             ; 1318:  Reject
        DB      01bH            , 0FFH          ; 1319:  optCommaExp->Accept
        DB      01H                             ; 1321:  Reject

        DB      01cH            , 01H           ; 1322:  optFilenum->1325
        DB      01H                             ; 1324:  Reject
        DB      063H            , 04H           ; 1325:  tkComma->1331
        DB      03H                             ; 1327:  EMIT(opStPut1)
        DW      opStPut1                        
        DB      00H                             ; 1330:  Accept
        DB      031H            , 0cH           ; 1331:  Exp->1345
        DB      063H            , 01H           ; 1333:  tkComma->1336
        DB      01H                             ; 1335:  Reject
        DB      036H            , 01H           ; 1336:  IdAryElemRef->1339
        DB      01H                             ; 1338:  Reject
        DB      03H                             ; 1339:  EMIT(opStPutRec2)
        DW      opStPutRec2                     
        DB      0fH             , 0FFH          ; 1342:  EMITFFFF->Accept
        DB      01H                             ; 1344:  Reject
        DB      063H            , 04H           ; 1345:  tkComma->1351
        DB      03H                             ; 1347:  EMIT(opStPut2)
        DW      opStPut2                        
        DB      00H                             ; 1350:  Accept
        DB      036H            , 01H           ; 1351:  IdAryElemRef->1354
        DB      01H                             ; 1353:  Reject
        DB      03H                             ; 1354:  EMIT(opStPutRec3)
        DW      opStPutRec3                     
        DB      0fH             , 0FFH          ; 1357:  EMITFFFF->Accept
        DB      01H                             ; 1359:  Reject

        DB      0dH             , 01H           ; 1360:  coordStep->1363
        DB      01H                             ; 1362:  Reject
        DB      063H            , 01H           ; 1363:  tkComma->1366
        DB      01H                             ; 1365:  Reject
        DB      037H            , 01H           ; 1366:  IdAryGetPut->1369
        DB      01H                             ; 1368:  Reject
        DB      03H                             ; 1369:  EMIT(opStGraphicsPut)
        DW      opStGraphicsPut                 
        DB      063H            , 03H           ; 1372:  tkComma->1377
        DB      0fH             , 0FFH          ; 1374:  EMITFFFF->Accept
        DB      01H                             ; 1376:  Reject
        DB      06fH            , 01dH          ; 1377:  tkAND->1408
        DB      0e0H, 012H      , 016H          ; 1379:  tkOR->1404
        DB      0e0H, 01fH      , 0fH           ; 1382:  tkPRESET->1400
        DB      0e0H, 021H      , 08H           ; 1385:  tkPSET->1396
        DB      0e0H, 068H      , 01H           ; 1388:  tkXOR->1392
        DB      01H                             ; 1391:  Reject
        DB      03H                             ; 1392:  EMIT(4)
        DW      4
        DB      00H                             ; 1395:  Accept
        DB      03H                             ; 1396:  EMIT(3)
        DW      3
        DB      00H                             ; 1399:  Accept
        DB      03H                             ; 1400:  EMIT(2)
        DW      2
        DB      00H                             ; 1403:  Accept
        DB      03H                             ; 1404:  EMIT(0)
        DW      0
        DB      00H                             ; 1407:  Accept
        DB      03H                             ; 1408:  EMIT(1)
        DW      1
        DB      00H                             ; 1411:  Accept

        DB      031H            , 04H           ; 1412:  Exp->1418
        DB      03H                             ; 1414:  EMIT(opStRandomize0)
        DW      opStRandomize0                  
        DB      00H                             ; 1417:  Accept
        DB      03H                             ; 1418:  EMIT(opStRandomize1)
        DW      opStRandomize1                  
        DB      00H                             ; 1421:  Accept

        DB      036H            , 01H           ; 1422:  IdAryElemRef->1425
        DB      01H                             ; 1424:  Reject
        DB      03H                             ; 1425:  EMIT(opStRead)
        DW      opStRead                        
        DB      063H            , 01H           ; 1428:  tkComma->1431
        DB      00H                             ; 1430:  Accept
        DB      036H            , 01H           ; 1431:  IdAryElemRef->1434
        DB      01H                             ; 1433:  Reject
        DB      03H                             ; 1434:  EMIT(opStRead)
        DW      opStRead                        
        DB      04H             , 0d5H          ; 1437:  empty->1428

        DB      0e0H, 039H      , 02H           ; 1439:  tkSHARED->1444
        DB      04H             , 06H           ; 1442:  empty->1450
        DB      023H            , 01H           ; 1444:  ACTIONidShared->1447
        DB      01H                             ; 1446:  Reject
        DB      03H                             ; 1447:  EMIT(opShared)
        DW      opShared                        
        DB      034H            , 01H           ; 1450:  IdAryRedim->1453
        DB      01H                             ; 1452:  Reject
        DB      063H            , 01H           ; 1453:  tkComma->1456
        DB      00H                             ; 1455:  Accept
        DB      034H            , 0dbH          ; 1456:  IdAryRedim->1453
        DB      01H                             ; 1458:  Reject

        DB      03H                             ; 1459:  EMIT(opStReset)
        DW      opStReset                       
        DB      00H                             ; 1462:  Accept

        DB      02H             , 01H           ; 1463:  MARK(1)
        DB      046H            , 01H           ; 1465:  LabLn->1468
        DB      00H                             ; 1467:  Accept
        DB      02H             , 02H           ; 1468:  MARK(2)
        DB      00H                             ; 1470:  Accept

        DB      02H             , 01H           ; 1471:  MARK(1)
        DB      048H            , 0cH           ; 1473:  Lit0->1487
        DB      046H            , 07H           ; 1475:  LabLn->1484
        DB      0e0H, 0bH       , 01H           ; 1477:  tkNEXT->1481
        DB      00H                             ; 1480:  Accept
        DB      02H             , 04H           ; 1481:  MARK(4)
        DB      00H                             ; 1483:  Accept
        DB      02H             , 02H           ; 1484:  MARK(2)
        DB      00H                             ; 1486:  Accept
        DB      02H             , 03H           ; 1487:  MARK(3)
        DB      00H                             ; 1489:  Accept

        DB      02H             , 01H           ; 1490:  MARK(1)
        DB      046H            , 01H           ; 1492:  LabLn->1495
        DB      00H                             ; 1494:  Accept
        DB      02H             , 02H           ; 1495:  MARK(2)
        DB      00H                             ; 1497:  Accept

        DB      031H            , 01H           ; 1498:  Exp->1501
        DB      01H                             ; 1500:  Reject
        DB      03H                             ; 1501:  EMIT(opStRmdir)
        DW      opStRmdir                       
        DB      00H                             ; 1504:  Accept

        DB      02H             , 01H           ; 1505:  MARK(1)
        DB      036H            , 01H           ; 1507:  IdAryElemRef->1510
        DB      01H                             ; 1509:  Reject
        DB      02H             , 02H           ; 1510:  MARK(2)
        DB      069H            , 01H           ; 1512:  tkEQ->1515
        DB      01H                             ; 1514:  Reject
        DB      031H            , 0FFH          ; 1515:  Exp->Accept
        DB      01H                             ; 1517:  Reject

        DB      04aH            , 06H           ; 1518:  Ln->1526
        DB      031H            , 01H           ; 1520:  Exp->1523
        DB      00H                             ; 1522:  Accept
        DB      02H             , 02H           ; 1523:  MARK(2)
        DB      00H                             ; 1525:  Accept
        DB      02H             , 01H           ; 1526:  MARK(1)
        DB      00H                             ; 1528:  Accept

        DB      04cH            , 0FFH          ; 1529:  NArgsMax4->Accept
        DB      01H                             ; 1531:  Reject

        DB      01cH            , 01H           ; 1532:  optFilenum->1535
        DB      01H                             ; 1534:  Reject
        DB      09H             , 01H           ; 1535:  commaExp->1538
        DB      01H                             ; 1537:  Reject
        DB      03H                             ; 1538:  EMIT(opStSeek)
        DW      opStSeek                        
        DB      00H                             ; 1541:  Accept

        DB      07dH            , 01H           ; 1542:  tkCASE->1545
        DB      01H                             ; 1544:  Reject
        DB      031H            , 01H           ; 1545:  Exp->1548
        DB      01H                             ; 1547:  Reject
        DB      03H                             ; 1548:  EMIT(opStSelectCase)
        DW      opStSelectCase                  
        DB      0fH             , 0FFH          ; 1551:  EMITFFFF->Accept
        DB      01H                             ; 1553:  Reject

        DB      03H                             ; 1554:  EMIT(opStShared)
        DW      opStShared                      
        DB      0fH             , 01H           ; 1557:  EMITFFFF->1560
        DB      01H                             ; 1559:  Reject
        DB      023H            , 01H           ; 1560:  ACTIONidShared->1563
        DB      01H                             ; 1562:  Reject
        DB      032H            , 01H           ; 1563:  IdAry->1566
        DB      01H                             ; 1565:  Reject
        DB      063H            , 01H           ; 1566:  tkComma->1569
        DB      00H                             ; 1568:  Accept
        DB      032H            , 0dbH          ; 1569:  IdAry->1566
        DB      01H                             ; 1571:  Reject

        DB      031H            , 0FFH          ; 1572:  Exp->Accept
        DB      00H                             ; 1574:  Accept

        DB      014H            , 01H           ; 1575:  fn1arg->1578
        DB      01H                             ; 1577:  Reject
        DB      03H                             ; 1578:  EMIT(opEvSignal)
        DW      opEvSignal                      
        DB      011H            , 0FFH          ; 1581:  evSwitch->Accept
        DB      01H                             ; 1583:  Reject

        DB      031H            , 04H           ; 1584:  Exp->1590
        DB      03H                             ; 1586:  EMIT(opStSleep0)
        DW      opStSleep0                      
        DB      00H                             ; 1589:  Accept
        DB      03H                             ; 1590:  EMIT(opStSleep1)
        DW      opStSleep1                      
        DB      00H                             ; 1593:  Accept

        DB      013H            , 01H           ; 1594:  expCommaExp->1597
        DB      01H                             ; 1596:  Reject
        DB      03H                             ; 1597:  EMIT(opStSound)
        DW      opStSound                       
        DB      00H                             ; 1600:  Accept

        DB      03H                             ; 1601:  EMIT(opStStatic)
        DW      opStStatic                      
        DB      0fH             , 01H           ; 1604:  EMITFFFF->1607
        DB      01H                             ; 1606:  Reject
        DB      024H            , 01H           ; 1607:  ACTIONidStatic->1610
        DB      01H                             ; 1609:  Reject
        DB      038H            , 01H           ; 1610:  IdAryI->1613
        DB      01H                             ; 1612:  Reject
        DB      063H            , 01H           ; 1613:  tkComma->1616
        DB      00H                             ; 1615:  Accept
        DB      038H            , 0dbH          ; 1616:  IdAryI->1613
        DB      01H                             ; 1618:  Reject

        DB      03H                             ; 1619:  EMIT(opStStop)
        DW      opStStop                        
        DB      03H                             ; 1622:  EMIT(opNop)
        DW      opNop                           
        DB      00H                             ; 1625:  Accept

        DB      014H            , 07H           ; 1626:  fn1arg->1635
        DB      0e0H, 0fH       , 0FFH          ; 1628:  tkON->Accept
        DB      0e0H, 0eH       , 0FFH          ; 1631:  tkOFF->Accept
        DB      01H                             ; 1634:  Reject
        DB      03H                             ; 1635:  EMIT(opEvStrig)
        DW      opEvStrig                       
        DB      011H            , 0FFH          ; 1638:  evSwitch->Accept
        DB      01H                             ; 1640:  Reject

        DB      030H            , 01H           ; 1641:  ErrIfNot1st->1644
        DB      01H                             ; 1643:  Reject
        DB      043H            , 01H           ; 1644:  IdSubDef->1647
        DB      01H                             ; 1646:  Reject
        DB      02H             , 03H           ; 1647:  MARK(3)
        DB      01dH            , 01H           ; 1649:  parms->1652
        DB      01H                             ; 1651:  Reject
        DB      0e0H, 043H      , 01H           ; 1652:  tkSTATIC->1656
        DB      00H                             ; 1655:  Accept
        DB      02H             , 04H           ; 1656:  MARK(4)
        DB      00H                             ; 1658:  Accept

        DB      036H            , 01H           ; 1659:  IdAryElemRef->1662
        DB      01H                             ; 1661:  Reject
        DB      063H            , 01H           ; 1662:  tkComma->1665
        DB      01H                             ; 1664:  Reject
        DB      036H            , 01H           ; 1665:  IdAryElemRef->1668
        DB      01H                             ; 1667:  Reject
        DB      03H                             ; 1668:  EMIT(opStSwap)
        DW      opStSwap                        
        DB      0fH             , 0FFH          ; 1671:  EMITFFFF->Accept
        DB      01H                             ; 1673:  Reject

        DB      03H                             ; 1674:  EMIT(opStSystem)
        DW      opStSystem                      
        DB      00H                             ; 1677:  Accept

        DB      069H            , 01H           ; 1678:  tkEQ->1681
        DB      01H                             ; 1680:  Reject
        DB      031H            , 01H           ; 1681:  Exp->1684
        DB      01H                             ; 1683:  Reject
        DB      03H                             ; 1684:  EMIT(opStTime_)
        DW      opStTime_                       
        DB      00H                             ; 1687:  Accept

        DB      03H                             ; 1688:  EMIT(opEvTimer0)
        DW      opEvTimer0                      
        DB      011H            , 0FFH          ; 1691:  evSwitch->Accept
        DB      01H                             ; 1693:  Reject

        DB      03H                             ; 1694:  EMIT(opStTroff)
        DW      opStTroff                       
        DB      00H                             ; 1697:  Accept

        DB      03H                             ; 1698:  EMIT(opStTron)
        DW      opStTron                        
        DB      00H                             ; 1701:  Accept

        DB      03H                             ; 1702:  EMIT(opStType)
        DW      opStType                        
        DB      0fH             , 01H           ; 1705:  EMITFFFF->1708
        DB      01H                             ; 1707:  Reject
        DB      03fH            , 0FFH          ; 1708:  IdType->Accept
        DB      01H                             ; 1710:  Reject

        DB      01cH            , 01H           ; 1711:  optFilenum->1714
        DB      01H                             ; 1713:  Reject
        DB      063H            , 01H           ; 1714:  tkComma->1717
        DB      00H                             ; 1716:  Accept
        DB      031H            , 09H           ; 1717:  Exp->1728
        DB      0e0H, 053H      , 01H           ; 1719:  tkTO->1723
        DB      01H                             ; 1722:  Reject
        DB      02H             , 03H           ; 1723:  MARK(3)
        DB      031H            , 0FFH          ; 1725:  Exp->Accept
        DB      01H                             ; 1727:  Reject
        DB      02H             , 01H           ; 1728:  MARK(1)
        DB      0e0H, 053H      , 01H           ; 1730:  tkTO->1734
        DB      00H                             ; 1733:  Accept
        DB      02H             , 02H           ; 1734:  MARK(2)
        DB      031H            , 0FFH          ; 1736:  Exp->Accept
        DB      01H                             ; 1738:  Reject

        DB      03H                             ; 1739:  EMIT(opEvUEvent)
        DW      opEvUEvent                      
        DB      011H            , 0FFH          ; 1742:  evSwitch->Accept
        DB      01H                             ; 1744:  Reject

        DB      0e0H, 020H      , 02cH          ; 1745:  tkPRINT->1792
        DB      0e0H, 033H      , 016H          ; 1748:  tkSCREEN->1773
        DB      016H            , 04H           ; 1751:  fn2arg->1757
        DB      03H                             ; 1753:  EMIT(opStView0)
        DW      opStView0                       
        DB      00H                             ; 1756:  Accept
        DB      064H            , 01H           ; 1757:  tkMinus->1760
        DB      01H                             ; 1759:  Reject
        DB      016H            , 01H           ; 1760:  fn2arg->1763
        DB      01H                             ; 1762:  Reject
        DB      0aH             , 01H           ; 1763:  commaOptExp->1766
        DB      01H                             ; 1765:  Reject
        DB      0aH             , 01H           ; 1766:  commaOptExp->1769
        DB      01H                             ; 1768:  Reject
        DB      03H                             ; 1769:  EMIT(opStView)
        DW      opStView                        
        DB      00H                             ; 1772:  Accept
        DB      016H            , 01H           ; 1773:  fn2arg->1776
        DB      01H                             ; 1775:  Reject
        DB      064H            , 01H           ; 1776:  tkMinus->1779
        DB      01H                             ; 1778:  Reject
        DB      016H            , 01H           ; 1779:  fn2arg->1782
        DB      01H                             ; 1781:  Reject
        DB      0aH             , 01H           ; 1782:  commaOptExp->1785
        DB      01H                             ; 1784:  Reject
        DB      0aH             , 01H           ; 1785:  commaOptExp->1788
        DB      01H                             ; 1787:  Reject
        DB      03H                             ; 1788:  EMIT(opStViewScreen)
        DW      opStViewScreen                  
        DB      00H                             ; 1791:  Accept
        DB      031H            , 04H           ; 1792:  Exp->1798
        DB      03H                             ; 1794:  EMIT(opStViewPrint0)
        DW      opStViewPrint0                  
        DB      00H                             ; 1797:  Accept
        DB      0e0H, 053H      , 01H           ; 1798:  tkTO->1802
        DB      01H                             ; 1801:  Reject
        DB      031H            , 01H           ; 1802:  Exp->1805
        DB      01H                             ; 1804:  Reject
        DB      03H                             ; 1805:  EMIT(opStViewPrint2)
        DW      opStViewPrint2                  
        DB      00H                             ; 1808:  Accept

        DB      031H            , 01H           ; 1809:  Exp->1812
        DB      01H                             ; 1811:  Reject
        DB      012H            , 0FFH          ; 1812:  exp12->Accept
        DB      01H                             ; 1814:  Reject

        DB      03H                             ; 1815:  EMIT(opStWend)
        DW      opStWend                        
        DB      0fH             , 0FFH          ; 1818:  EMITFFFF->Accept
        DB      01H                             ; 1820:  Reject

        DB      031H            , 01H           ; 1821:  Exp->1824
        DB      01H                             ; 1823:  Reject
        DB      03H                             ; 1824:  EMIT(opStWhile)
        DW      opStWhile                       
        DB      0fH             , 0FFH          ; 1827:  EMITFFFF->Accept
        DB      01H                             ; 1829:  Reject

        DB      056H            , 01dH          ; 1830:  tkLbs->1861
        DB      0deH            , 014H          ; 1832:  tkLPRINT->1854
        DB      031H            , 09H           ; 1834:  Exp->1845
        DB      063H            , 01H           ; 1836:  tkComma->1839
        DB      01H                             ; 1838:  Reject
        DB      03H                             ; 1839:  EMIT(opUndef)
        DW      opUndef                         
        DB      031H            , 06H           ; 1842:  Exp->1850
        DB      01H                             ; 1844:  Reject
        DB      09H             , 03H           ; 1845:  commaExp->1850
        DB      03H                             ; 1847:  EMIT(opUndef)
        DW      opUndef                         
        DB      03H                             ; 1850:  EMIT(opStWidth2)
        DW      opStWidth2                      
        DB      00H                             ; 1853:  Accept
        DB      031H            , 01H           ; 1854:  Exp->1857
        DB      01H                             ; 1856:  Reject
        DB      03H                             ; 1857:  EMIT(opStWidthLprint)
        DW      opStWidthLprint                 
        DB      00H                             ; 1860:  Accept
        DB      031H            , 01H           ; 1861:  Exp->1864
        DB      01H                             ; 1863:  Reject
        DB      03H                             ; 1864:  EMIT(opLbs)
        DW      opLbs                           
        DB      063H            , 01H           ; 1867:  tkComma->1870
        DB      01H                             ; 1869:  Reject
        DB      031H            , 01H           ; 1870:  Exp->1873
        DB      01H                             ; 1872:  Reject
        DB      03H                             ; 1873:  EMIT(opStWidthFile)
        DW      opStWidthFile                   
        DB      00H                             ; 1876:  Accept

        DB      0e0H, 033H      , 010H          ; 1877:  tkSCREEN->1896
        DB      016H            , 04H           ; 1880:  fn2arg->1886
        DB      03H                             ; 1882:  EMIT(opStWindow0)
        DW      opStWindow0                     
        DB      00H                             ; 1885:  Accept
        DB      064H            , 01H           ; 1886:  tkMinus->1889
        DB      01H                             ; 1888:  Reject
        DB      016H            , 01H           ; 1889:  fn2arg->1892
        DB      01H                             ; 1891:  Reject
        DB      03H                             ; 1892:  EMIT(opStWindow)
        DW      opStWindow                      
        DB      00H                             ; 1895:  Accept
        DB      016H            , 01H           ; 1896:  fn2arg->1899
        DB      01H                             ; 1898:  Reject
        DB      064H            , 01H           ; 1899:  tkMinus->1902
        DB      01H                             ; 1901:  Reject
        DB      016H            , 01H           ; 1902:  fn2arg->1905
        DB      01H                             ; 1904:  Reject
        DB      03H                             ; 1905:  EMIT(opStWindowScreen)
        DW      opStWindowScreen                
        DB      00H                             ; 1908:  Accept

        DB      03H                             ; 1909:  EMIT(opStWrite)
        DW      opStWrite                       
        DB      019H            , 00H           ; 1912:  lbsExpComma->1914
        DB      031H            , 04H           ; 1914:  Exp->1920
        DB      03H                             ; 1916:  EMIT(opPrintEos)
        DW      opPrintEos                      
        DB      00H                             ; 1919:  Accept
        DB      063H            , 06H           ; 1920:  tkComma->1928
        DB      067H            , 04H           ; 1922:  tkSColon->1928
        DB      03H                             ; 1924:  EMIT(opPrintItemEos)
        DW      opPrintItemEos                  
        DB      00H                             ; 1927:  Accept
        DB      03H                             ; 1928:  EMIT(opPrintItemComma)
        DW      opPrintItemComma                
        DB      031H            , 0d3H          ; 1931:  Exp->1920
        DB      01H                             ; 1933:  Reject

        DB      014H            , 01H           ; 1934:  fn1arg->1937
        DB      01H                             ; 1936:  Reject
        DB      03H                             ; 1937:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 1940:  Accept

        DB      014H            , 01H           ; 1941:  fn1arg->1944
        DB      01H                             ; 1943:  Reject
        DB      03H                             ; 1944:  EMIT(opFnAsc)
        DW      opFnAsc                         
        DB      00H                             ; 1947:  Accept

        DB      014H            , 01H           ; 1948:  fn1arg->1951
        DB      01H                             ; 1950:  Reject
        DB      03H                             ; 1951:  EMIT(opFnAtn)
        DW      opFnAtn                         
        DB      00H                             ; 1954:  Accept

        DB      014H            , 01H           ; 1955:  fn1arg->1958
        DB      01H                             ; 1957:  Reject
        DB      03H                             ; 1958:  EMIT(opCoerce,ET_R8)
        DW      ET_R8*(OPCODE_MASK+1)+opCoerce
        DB      00H                             ; 1961:  Accept

        DB      014H            , 01H           ; 1962:  fn1arg->1965
        DB      01H                             ; 1964:  Reject
        DB      03H                             ; 1965:  EMIT(opFnChr_)
        DW      opFnChr_                        
        DB      00H                             ; 1968:  Accept

        DB      014H            , 01H           ; 1969:  fn1arg->1972
        DB      01H                             ; 1971:  Reject
        DB      03H                             ; 1972:  EMIT(opCoerce,ET_I2)
        DW      ET_I2*(OPCODE_MASK+1)+opCoerce
        DB      00H                             ; 1975:  Accept

        DB      014H            , 01H           ; 1976:  fn1arg->1979
        DB      01H                             ; 1978:  Reject
        DB      03H                             ; 1979:  EMIT(opCoerce,ET_I4)
        DW      ET_I4*(OPCODE_MASK+1)+opCoerce
        DB      00H                             ; 1982:  Accept

        DB      03H                             ; 1983:  EMIT(opFnCommand_)
        DW      opFnCommand_                    
        DB      00H                             ; 1986:  Accept

        DB      014H            , 01H           ; 1987:  fn1arg->1990
        DB      01H                             ; 1989:  Reject
        DB      03H                             ; 1990:  EMIT(opFnCos)
        DW      opFnCos                         
        DB      00H                             ; 1993:  Accept

        DB      014H            , 01H           ; 1994:  fn1arg->1997
        DB      01H                             ; 1996:  Reject
        DB      03H                             ; 1997:  EMIT(opCoerce,ET_R4)
        DW      ET_R4*(OPCODE_MASK+1)+opCoerce
        DB      00H                             ; 2000:  Accept

        DB      03H                             ; 2001:  EMIT(opFnCsrlin)
        DW      opFnCsrlin                      
        DB      00H                             ; 2004:  Accept

        DB      014H            , 01H           ; 2005:  fn1arg->2008
        DB      01H                             ; 2007:  Reject
        DB      03H                             ; 2008:  EMIT(opFnCvd)
        DW      opFnCvd                         
        DB      00H                             ; 2011:  Accept

        DB      014H            , 01H           ; 2012:  fn1arg->2015
        DB      01H                             ; 2014:  Reject
        DB      03H                             ; 2015:  EMIT(opFnCvdmbf)
        DW      opFnCvdmbf                      
        DB      00H                             ; 2018:  Accept

        DB      014H            , 01H           ; 2019:  fn1arg->2022
        DB      01H                             ; 2021:  Reject
        DB      03H                             ; 2022:  EMIT(opFnCvi)
        DW      opFnCvi                         
        DB      00H                             ; 2025:  Accept

        DB      014H            , 01H           ; 2026:  fn1arg->2029
        DB      01H                             ; 2028:  Reject
        DB      03H                             ; 2029:  EMIT(opFnCvl)
        DW      opFnCvl                         
        DB      00H                             ; 2032:  Accept

        DB      014H            , 01H           ; 2033:  fn1arg->2036
        DB      01H                             ; 2035:  Reject
        DB      03H                             ; 2036:  EMIT(opFnCvs)
        DW      opFnCvs                         
        DB      00H                             ; 2039:  Accept

        DB      014H            , 01H           ; 2040:  fn1arg->2043
        DB      01H                             ; 2042:  Reject
        DB      03H                             ; 2043:  EMIT(opFnCvsmbf)
        DW      opFnCvsmbf                      
        DB      00H                             ; 2046:  Accept

        DB      03H                             ; 2047:  EMIT(opFnDate_)
        DW      opFnDate_                       
        DB      00H                             ; 2050:  Accept

        DB      014H            , 01H           ; 2051:  fn1arg->2054
        DB      01H                             ; 2053:  Reject
        DB      03H                             ; 2054:  EMIT(opFnEnviron_)
        DW      opFnEnviron_                    
        DB      00H                             ; 2057:  Accept

        DB      014H            , 01H           ; 2058:  fn1arg->2061
        DB      01H                             ; 2060:  Reject
        DB      03H                             ; 2061:  EMIT(opFnEof)
        DW      opFnEof                         
        DB      00H                             ; 2064:  Accept

        DB      03H                             ; 2065:  EMIT(opFnErdev)
        DW      opFnErdev                       
        DB      00H                             ; 2068:  Accept

        DB      03H                             ; 2069:  EMIT(opFnErdev_)
        DW      opFnErdev_                      
        DB      00H                             ; 2072:  Accept

        DB      03H                             ; 2073:  EMIT(opFnErl)
        DW      opFnErl                         
        DB      00H                             ; 2076:  Accept

        DB      03H                             ; 2077:  EMIT(opFnErr)
        DW      opFnErr                         
        DB      00H                             ; 2080:  Accept

        DB      014H            , 01H           ; 2081:  fn1arg->2084
        DB      01H                             ; 2083:  Reject
        DB      03H                             ; 2084:  EMIT(opFnExp)
        DW      opFnExp                         
        DB      00H                             ; 2087:  Accept

        DB      016H            , 01H           ; 2088:  fn2arg->2091
        DB      01H                             ; 2090:  Reject
        DB      03H                             ; 2091:  EMIT(opFnFileattr)
        DW      opFnFileattr                    
        DB      00H                             ; 2094:  Accept

        DB      014H            , 01H           ; 2095:  fn1arg->2098
        DB      01H                             ; 2097:  Reject
        DB      03H                             ; 2098:  EMIT(opFnFix)
        DW      opFnFix                         
        DB      00H                             ; 2101:  Accept

        DB      014H            , 01H           ; 2102:  fn1arg->2105
        DB      01H                             ; 2104:  Reject
        DB      03H                             ; 2105:  EMIT(opFnFre)
        DW      opFnFre                         
        DB      00H                             ; 2108:  Accept

        DB      03H                             ; 2109:  EMIT(opFnFreefile)
        DW      opFnFreefile                    
        DB      00H                             ; 2112:  Accept

        DB      014H            , 01H           ; 2113:  fn1arg->2116
        DB      01H                             ; 2115:  Reject
        DB      03H                             ; 2116:  EMIT(opFnHex_)
        DW      opFnHex_                        
        DB      00H                             ; 2119:  Accept

        DB      03H                             ; 2120:  EMIT(opFnInkey_)
        DW      opFnInkey_                      
        DB      00H                             ; 2123:  Accept

        DB      014H            , 01H           ; 2124:  fn1arg->2127
        DB      01H                             ; 2126:  Reject
        DB      03H                             ; 2127:  EMIT(opFnInp)
        DW      opFnInp                         
        DB      00H                             ; 2130:  Accept

        DB      05fH            , 01H           ; 2131:  tkLParen->2134
        DB      01H                             ; 2133:  Reject
        DB      031H            , 01H           ; 2134:  Exp->2137
        DB      01H                             ; 2136:  Reject
        DB      063H            , 02H           ; 2137:  tkComma->2141
        DB      04H             , 03H           ; 2139:  empty->2144
        DB      01cH            , 01H           ; 2141:  optFilenum->2144
        DB      01H                             ; 2143:  Reject
        DB      060H            , 0FFH          ; 2144:  tkRParen->Accept
        DB      01H                             ; 2146:  Reject

        DB      017H            , 0FFH          ; 2147:  fn23arg->Accept
        DB      01H                             ; 2149:  Reject

        DB      014H            , 01H           ; 2150:  fn1arg->2153
        DB      01H                             ; 2152:  Reject
        DB      03H                             ; 2153:  EMIT(opFnInt)
        DW      opFnInt                         
        DB      00H                             ; 2156:  Accept

        DB      05fH            , 01H           ; 2157:  tkLParen->2160
        DB      01H                             ; 2159:  Reject
        DB      01cH            , 01H           ; 2160:  optFilenum->2163
        DB      01H                             ; 2162:  Reject
        DB      060H            , 01H           ; 2163:  tkRParen->2166
        DB      01H                             ; 2165:  Reject
        DB      03H                             ; 2166:  EMIT(opFnIoctl_)
        DW      opFnIoctl_                      
        DB      00H                             ; 2169:  Accept

        DB      018H            , 0FFH          ; 2170:  fnBoundArg->Accept
        DB      01H                             ; 2172:  Reject

        DB      014H            , 01H           ; 2173:  fn1arg->2176
        DB      01H                             ; 2175:  Reject
        DB      03H                             ; 2176:  EMIT(opFnLcase_)
        DW      opFnLcase_                      
        DB      00H                             ; 2179:  Accept

        DB      016H            , 01H           ; 2180:  fn2arg->2183
        DB      01H                             ; 2182:  Reject
        DB      03H                             ; 2183:  EMIT(opFnLeft_)
        DW      opFnLeft_                       
        DB      00H                             ; 2186:  Accept

        DB      014H            , 01H           ; 2187:  fn1arg->2190
        DB      01H                             ; 2189:  Reject
        DB      03H                             ; 2190:  EMIT(opFnLen)
        DW      opFnLen                         
        DB      0fH             , 0FFH          ; 2193:  EMITFFFF->Accept
        DB      01H                             ; 2195:  Reject

        DB      014H            , 01H           ; 2196:  fn1arg->2199
        DB      01H                             ; 2198:  Reject
        DB      03H                             ; 2199:  EMIT(opFnLoc)
        DW      opFnLoc                         
        DB      00H                             ; 2202:  Accept

        DB      014H            , 01H           ; 2203:  fn1arg->2206
        DB      01H                             ; 2205:  Reject
        DB      03H                             ; 2206:  EMIT(opFnLof)
        DW      opFnLof                         
        DB      00H                             ; 2209:  Accept

        DB      014H            , 01H           ; 2210:  fn1arg->2213
        DB      01H                             ; 2212:  Reject
        DB      03H                             ; 2213:  EMIT(opFnLog)
        DW      opFnLog                         
        DB      00H                             ; 2216:  Accept

        DB      014H            , 01H           ; 2217:  fn1arg->2220
        DB      01H                             ; 2219:  Reject
        DB      03H                             ; 2220:  EMIT(opFnLpos)
        DW      opFnLpos                        
        DB      00H                             ; 2223:  Accept

        DB      014H            , 01H           ; 2224:  fn1arg->2227
        DB      01H                             ; 2226:  Reject
        DB      03H                             ; 2227:  EMIT(opFnLtrim_)
        DW      opFnLtrim_                      
        DB      00H                             ; 2230:  Accept

        DB      017H            , 0FFH          ; 2231:  fn23arg->Accept
        DB      01H                             ; 2233:  Reject

        DB      014H            , 01H           ; 2234:  fn1arg->2237
        DB      01H                             ; 2236:  Reject
        DB      03H                             ; 2237:  EMIT(opFnMkd_)
        DW      opFnMkd_                        
        DB      00H                             ; 2240:  Accept

        DB      014H            , 01H           ; 2241:  fn1arg->2244
        DB      01H                             ; 2243:  Reject
        DB      03H                             ; 2244:  EMIT(opFnMkdmbf_)
        DW      opFnMkdmbf_                     
        DB      00H                             ; 2247:  Accept

        DB      014H            , 01H           ; 2248:  fn1arg->2251
        DB      01H                             ; 2250:  Reject
        DB      03H                             ; 2251:  EMIT(opFnMki_)
        DW      opFnMki_                        
        DB      00H                             ; 2254:  Accept

        DB      014H            , 01H           ; 2255:  fn1arg->2258
        DB      01H                             ; 2257:  Reject
        DB      03H                             ; 2258:  EMIT(opFnMkl_)
        DW      opFnMkl_                        
        DB      00H                             ; 2261:  Accept

        DB      014H            , 01H           ; 2262:  fn1arg->2265
        DB      01H                             ; 2264:  Reject
        DB      03H                             ; 2265:  EMIT(opFnMks_)
        DW      opFnMks_                        
        DB      00H                             ; 2268:  Accept

        DB      014H            , 01H           ; 2269:  fn1arg->2272
        DB      01H                             ; 2271:  Reject
        DB      03H                             ; 2272:  EMIT(opFnMksmbf_)
        DW      opFnMksmbf_                     
        DB      00H                             ; 2275:  Accept

        DB      014H            , 01H           ; 2276:  fn1arg->2279
        DB      01H                             ; 2278:  Reject
        DB      03H                             ; 2279:  EMIT(opFnOct_)
        DW      opFnOct_                        
        DB      00H                             ; 2282:  Accept

        DB      014H            , 01H           ; 2283:  fn1arg->2286
        DB      01H                             ; 2285:  Reject
        DB      03H                             ; 2286:  EMIT(opFnPeek)
        DW      opFnPeek                        
        DB      00H                             ; 2289:  Accept

        DB      014H            , 01H           ; 2290:  fn1arg->2293
        DB      01H                             ; 2292:  Reject
        DB      03H                             ; 2293:  EMIT(opFnPen)
        DW      opFnPen                         
        DB      00H                             ; 2296:  Accept

        DB      014H            , 01H           ; 2297:  fn1arg->2300
        DB      01H                             ; 2299:  Reject
        DB      03H                             ; 2300:  EMIT(opFnPlay)
        DW      opFnPlay                        
        DB      00H                             ; 2303:  Accept

        DB      016H            , 01H           ; 2304:  fn2arg->2307
        DB      01H                             ; 2306:  Reject
        DB      03H                             ; 2307:  EMIT(opFnPmap)
        DW      opFnPmap                        
        DB      00H                             ; 2310:  Accept

        DB      015H            , 0FFH          ; 2311:  fn12arg->Accept
        DB      01H                             ; 2313:  Reject

        DB      014H            , 01H           ; 2314:  fn1arg->2317
        DB      01H                             ; 2316:  Reject
        DB      03H                             ; 2317:  EMIT(opFnPos)
        DW      opFnPos                         
        DB      00H                             ; 2320:  Accept

        DB      016H            , 01H           ; 2321:  fn2arg->2324
        DB      01H                             ; 2323:  Reject
        DB      03H                             ; 2324:  EMIT(opFnRight_)
        DW      opFnRight_                      
        DB      00H                             ; 2327:  Accept

        DB      014H            , 0FFH          ; 2328:  fn1arg->Accept
        DB      00H                             ; 2330:  Accept

        DB      014H            , 01H           ; 2331:  fn1arg->2334
        DB      01H                             ; 2333:  Reject
        DB      03H                             ; 2334:  EMIT(opFnRtrim_)
        DW      opFnRtrim_                      
        DB      00H                             ; 2337:  Accept

        DB      05fH            , 01H           ; 2338:  tkLParen->2341
        DB      01H                             ; 2340:  Reject
        DB      036H            , 01H           ; 2341:  IdAryElemRef->2344
        DB      01H                             ; 2343:  Reject
        DB      060H            , 01H           ; 2344:  tkRParen->2347
        DB      01H                             ; 2346:  Reject
        DB      03H                             ; 2347:  EMIT(opFnSadd)
        DW      opFnSadd                        
        DB      00H                             ; 2350:  Accept

        DB      017H            , 0FFH          ; 2351:  fn23arg->Accept
        DB      01H                             ; 2353:  Reject

        DB      014H            , 01H           ; 2354:  fn1arg->2357
        DB      01H                             ; 2356:  Reject
        DB      03H                             ; 2357:  EMIT(opFnSeek)
        DW      opFnSeek                        
        DB      00H                             ; 2360:  Accept

        DB      014H            , 01H           ; 2361:  fn1arg->2364
        DB      01H                             ; 2363:  Reject
        DB      03H                             ; 2364:  EMIT(opFnSetmem)
        DW      opFnSetmem                      
        DB      00H                             ; 2367:  Accept

        DB      014H            , 01H           ; 2368:  fn1arg->2371
        DB      01H                             ; 2370:  Reject
        DB      03H                             ; 2371:  EMIT(opFnSgn)
        DW      opFnSgn                         
        DB      00H                             ; 2374:  Accept

        DB      014H            , 01H           ; 2375:  fn1arg->2378
        DB      01H                             ; 2377:  Reject
        DB      03H                             ; 2378:  EMIT(opFnShell)
        DW      opFnShell                       
        DB      00H                             ; 2381:  Accept

        DB      014H            , 01H           ; 2382:  fn1arg->2385
        DB      01H                             ; 2384:  Reject
        DB      03H                             ; 2385:  EMIT(opFnSin)
        DW      opFnSin                         
        DB      00H                             ; 2388:  Accept

        DB      014H            , 01H           ; 2389:  fn1arg->2392
        DB      01H                             ; 2391:  Reject
        DB      03H                             ; 2392:  EMIT(opFnSpace_)
        DW      opFnSpace_                      
        DB      00H                             ; 2395:  Accept

        DB      014H            , 01H           ; 2396:  fn1arg->2399
        DB      01H                             ; 2398:  Reject
        DB      03H                             ; 2399:  EMIT(opFnSqr)
        DW      opFnSqr                         
        DB      00H                             ; 2402:  Accept

        DB      014H            , 01H           ; 2403:  fn1arg->2406
        DB      01H                             ; 2405:  Reject
        DB      03H                             ; 2406:  EMIT(opFnStick)
        DW      opFnStick                       
        DB      00H                             ; 2409:  Accept

        DB      014H            , 01H           ; 2410:  fn1arg->2413
        DB      01H                             ; 2412:  Reject
        DB      03H                             ; 2413:  EMIT(opFnStr_)
        DW      opFnStr_                        
        DB      00H                             ; 2416:  Accept

        DB      014H            , 01H           ; 2417:  fn1arg->2420
        DB      01H                             ; 2419:  Reject
        DB      03H                             ; 2420:  EMIT(opFnStrig)
        DW      opFnStrig                       
        DB      00H                             ; 2423:  Accept

        DB      016H            , 01H           ; 2424:  fn2arg->2427
        DB      01H                             ; 2426:  Reject
        DB      03H                             ; 2427:  EMIT(opFnString_)
        DW      opFnString_                     
        DB      00H                             ; 2430:  Accept

        DB      014H            , 01H           ; 2431:  fn1arg->2434
        DB      01H                             ; 2433:  Reject
        DB      03H                             ; 2434:  EMIT(opFnTan)
        DW      opFnTan                         
        DB      00H                             ; 2437:  Accept

        DB      03H                             ; 2438:  EMIT(opFnTime_)
        DW      opFnTime_                       
        DB      00H                             ; 2441:  Accept

        DB      03H                             ; 2442:  EMIT(opFnTimer)
        DW      opFnTimer                       
        DB      00H                             ; 2445:  Accept

        DB      018H            , 0FFH          ; 2446:  fnBoundArg->Accept
        DB      01H                             ; 2448:  Reject

        DB      014H            , 01H           ; 2449:  fn1arg->2452
        DB      01H                             ; 2451:  Reject
        DB      03H                             ; 2452:  EMIT(opFnUcase_)
        DW      opFnUcase_                      
        DB      00H                             ; 2455:  Accept

        DB      014H            , 01H           ; 2456:  fn1arg->2459
        DB      01H                             ; 2458:  Reject
        DB      03H                             ; 2459:  EMIT(opFnVal)
        DW      opFnVal                         
        DB      00H                             ; 2462:  Accept

        DB      05fH            , 01H           ; 2463:  tkLParen->2466
        DB      01H                             ; 2465:  Reject
        DB      036H            , 01H           ; 2466:  IdAryElemRef->2469
        DB      01H                             ; 2468:  Reject
        DB      060H            , 01H           ; 2469:  tkRParen->2472
        DB      01H                             ; 2471:  Reject
        DB      03H                             ; 2472:  EMIT(opFnVarptr)
        DW      opFnVarptr                      
        DB      00H                             ; 2475:  Accept

        DB      05fH            , 01H           ; 2476:  tkLParen->2479
        DB      01H                             ; 2478:  Reject
        DB      036H            , 01H           ; 2479:  IdAryElemRef->2482
        DB      01H                             ; 2481:  Reject
        DB      060H            , 01H           ; 2482:  tkRParen->2485
        DB      01H                             ; 2484:  Reject
        DB      03H                             ; 2485:  EMIT(opFnVarptr_)
        DW      opFnVarptr_                     
        DB      0fH             , 0FFH          ; 2488:  EMITFFFF->Accept
        DB      01H                             ; 2490:  Reject

        DB      05fH            , 01H           ; 2491:  tkLParen->2494
        DB      01H                             ; 2493:  Reject
        DB      036H            , 01H           ; 2494:  IdAryElemRef->2497
        DB      01H                             ; 2496:  Reject
        DB      060H            , 01H           ; 2497:  tkRParen->2500
        DB      01H                             ; 2499:  Reject
        DB      03H                             ; 2500:  EMIT(opFnVarseg)
        DW      opFnVarseg                      
        DB      00H                             ; 2503:  Accept

        DB      0c8H            , 027H          ; 2504:  tkINTEGER->2545
        DB      0dbH            , 01eH          ; 2506:  tkLONG->2538
        DB      0e0H, 03dH      , 014H          ; 2508:  tkSINGLE->2531
        DB      0a2H            , 0bH           ; 2511:  tkDOUBLE->2524
        DB      0e0H, 049H      , 01H           ; 2513:  tkSTRING->2517
        DB      01H                             ; 2516:  Reject
        DB      03H                             ; 2517:  EMIT(opAsTypeExp)
        DW      opAsTypeExp                     
        DB      03H                             ; 2520:  EMIT(ET_SD)
        DW      ET_SD                           
        DB      00H                             ; 2523:  Accept
        DB      03H                             ; 2524:  EMIT(opAsTypeExp)
        DW      opAsTypeExp                     
        DB      03H                             ; 2527:  EMIT(ET_R8)
        DW      ET_R8                           
        DB      00H                             ; 2530:  Accept
        DB      03H                             ; 2531:  EMIT(opAsTypeExp)
        DW      opAsTypeExp                     
        DB      03H                             ; 2534:  EMIT(ET_R4)
        DW      ET_R4                           
        DB      00H                             ; 2537:  Accept
        DB      03H                             ; 2538:  EMIT(opAsTypeExp)
        DW      opAsTypeExp                     
        DB      03H                             ; 2541:  EMIT(ET_I4)
        DW      ET_I4                           
        DB      00H                             ; 2544:  Accept
        DB      03H                             ; 2545:  EMIT(opAsTypeExp)
        DW      opAsTypeExp                     
        DB      03H                             ; 2548:  EMIT(ET_I2)
        DW      ET_I2                           
        DB      00H                             ; 2551:  Accept

        DB      05H             , 0FFH          ; 2552:  AsClausePrim->Accept
        DB      03H                             ; 2554:  EMIT(opAsType)
        DW      opAsType                        
        DB      03fH            , 0FFH          ; 2557:  IdType->Accept
        DB      01H                             ; 2559:  Reject

        DB      05H             , 0FFH          ; 2560:  AsClausePrim->Accept
        DB      070H            , 06H           ; 2562:  tkANY->2570
        DB      03H                             ; 2564:  EMIT(opAsType)
        DW      opAsType                        
        DB      03fH            , 0FFH          ; 2567:  IdType->Accept
        DB      01H                             ; 2569:  Reject
        DB      03H                             ; 2570:  EMIT(opAsTypeExp)
        DW      opAsTypeExp                     
        DB      03H                             ; 2573:  EMIT(ET_IMP)
        DW      ET_IMP                          
        DB      00H                             ; 2576:  Accept

        DB      031H            , 05H           ; 2577:  Exp->2584
        DB      0cbH            , 00H           ; 2579:  tkIS->2581
        DB      026H            , 0FFH          ; 2581:  CaseRelation->Accept
        DB      01H                             ; 2583:  Reject
        DB      0e0H, 053H      , 04H           ; 2584:  tkTO->2591
        DB      03H                             ; 2587:  EMIT(opStCase)
        DW      opStCase                        
        DB      00H                             ; 2590:  Accept
        DB      031H            , 01H           ; 2591:  Exp->2594
        DB      01H                             ; 2593:  Reject
        DB      03H                             ; 2594:  EMIT(opStCaseTo)
        DW      opStCaseTo                      
        DB      00H                             ; 2597:  Accept

        DB      063H            , 01H           ; 2598:  tkComma->2601
        DB      01H                             ; 2600:  Reject
        DB      031H            , 0FFH          ; 2601:  Exp->Accept
        DB      01H                             ; 2603:  Reject

        DB      0bH             , 0FFH          ; 2604:  commaOptExpNil->Accept
        DB      03H                             ; 2606:  EMIT(opUndef)
        DW      opUndef                         
        DB      00H                             ; 2609:  Accept

        DB      027H            , 01H           ; 2610:  CommaNoEos->2613
        DB      01H                             ; 2612:  Reject
        DB      031H            , 0FFH          ; 2613:  Exp->Accept
        DB      03H                             ; 2615:  EMIT(opUndef)
        DW      opUndef                         
        DB      00H                             ; 2618:  Accept

        DB      027H            , 01H           ; 2619:  CommaNoEos->2622
        DB      01H                             ; 2621:  Reject
        DB      031H            , 0FFH          ; 2622:  Exp->Accept
        DB      03H                             ; 2624:  EMIT(opNull)
        DW      opNull                          
        DB      00H                             ; 2627:  Accept

        DB      0e0H, 044H      , 07H           ; 2628:  tkSTEP->2638
        DB      016H            , 01H           ; 2631:  fn2arg->2634
        DB      01H                             ; 2633:  Reject
        DB      03H                             ; 2634:  EMIT(opCoord)
        DW      opCoord                         
        DB      00H                             ; 2637:  Accept
        DB      016H            , 01H           ; 2638:  fn2arg->2641
        DB      01H                             ; 2640:  Reject
        DB      03H                             ; 2641:  EMIT(opCoordStep)
        DW      opCoordStep                     
        DB      00H                             ; 2644:  Accept

        DB      0e0H, 044H      , 07H           ; 2645:  tkSTEP->2655
        DB      016H            , 01H           ; 2648:  fn2arg->2651
        DB      01H                             ; 2650:  Reject
        DB      03H                             ; 2651:  EMIT(opCoordSecond)
        DW      opCoordSecond                   
        DB      00H                             ; 2654:  Accept
        DB      016H            , 01H           ; 2655:  fn2arg->2658
        DB      01H                             ; 2657:  Reject
        DB      03H                             ; 2658:  EMIT(opCoordStepSecond)
        DW      opCoordStepSecond               
        DB      00H                             ; 2661:  Accept

        DB      03H                             ; 2662:  EMIT(UNDEFINED)
        DW      UNDEFINED                       
        DB      00H                             ; 2665:  Accept

        DB      08aH            , 040H          ; 2666:  tkCOM->2732
        DB      0ccH            , 037H          ; 2668:  tkKEY->2725
        DB      0e0H, 019H      , 030H          ; 2670:  tkPEN->2721
        DB      0e0H, 01aH      , 026H          ; 2673:  tkPLAY->2714
        DB      0e0H, 03bH      , 01cH          ; 2676:  tkSIGNAL->2707
        DB      0e0H, 048H      , 012H          ; 2679:  tkSTRIG->2700
        DB      0e0H, 052H      , 08H           ; 2682:  tkTIMER->2693
        DB      0e0H, 059H      , 01H           ; 2685:  tkUEVENT->2689
        DB      01H                             ; 2688:  Reject
        DB      03H                             ; 2689:  EMIT(opEvUEvent)
        DW      opEvUEvent                      
        DB      00H                             ; 2692:  Accept
        DB      014H            , 01H           ; 2693:  fn1arg->2696
        DB      01H                             ; 2695:  Reject
        DB      03H                             ; 2696:  EMIT(opEvTimer1)
        DW      opEvTimer1                      
        DB      00H                             ; 2699:  Accept
        DB      014H            , 01H           ; 2700:  fn1arg->2703
        DB      01H                             ; 2702:  Reject
        DB      03H                             ; 2703:  EMIT(opEvStrig)
        DW      opEvStrig                       
        DB      00H                             ; 2706:  Accept
        DB      014H            , 01H           ; 2707:  fn1arg->2710
        DB      01H                             ; 2709:  Reject
        DB      03H                             ; 2710:  EMIT(opEvSignal)
        DW      opEvSignal                      
        DB      00H                             ; 2713:  Accept
        DB      014H            , 01H           ; 2714:  fn1arg->2717
        DB      01H                             ; 2716:  Reject
        DB      03H                             ; 2717:  EMIT(opEvPlay1)
        DW      opEvPlay1                       
        DB      00H                             ; 2720:  Accept
        DB      03H                             ; 2721:  EMIT(opEvPen)
        DW      opEvPen                         
        DB      00H                             ; 2724:  Accept
        DB      014H            , 01H           ; 2725:  fn1arg->2728
        DB      01H                             ; 2727:  Reject
        DB      03H                             ; 2728:  EMIT(opEvKey)
        DW      opEvKey                         
        DB      00H                             ; 2731:  Accept
        DB      014H            , 01H           ; 2732:  fn1arg->2735
        DB      01H                             ; 2734:  Reject
        DB      03H                             ; 2735:  EMIT(opEvCom)
        DW      opEvCom                         
        DB      00H                             ; 2738:  Accept

        DB      0e0H, 0fH       , 0fH           ; 2739:  tkON->2757
        DB      0e0H, 0eH       , 08H           ; 2742:  tkOFF->2753
        DB      0e0H, 046H      , 01H           ; 2745:  tkSTOP->2749
        DB      01H                             ; 2748:  Reject
        DB      03H                             ; 2749:  EMIT(opEvStop)
        DW      opEvStop                        
        DB      00H                             ; 2752:  Accept
        DB      03H                             ; 2753:  EMIT(opEvOff)
        DW      opEvOff                         
        DB      00H                             ; 2756:  Accept
        DB      03H                             ; 2757:  EMIT(opEvOn)
        DW      opEvOn                          
        DB      00H                             ; 2760:  Accept

        DB      09H             , 01H           ; 2761:  commaExp->2764
        DB      01H                             ; 2763:  Reject
        DB      01bH            , 0FFH          ; 2764:  optCommaExp->Accept
        DB      01H                             ; 2766:  Reject

        DB      031H            , 01H           ; 2767:  Exp->2770
        DB      01H                             ; 2769:  Reject
        DB      063H            , 01H           ; 2770:  tkComma->2773
        DB      01H                             ; 2772:  Reject
        DB      031H            , 0FFH          ; 2773:  Exp->Accept
        DB      01H                             ; 2775:  Reject

        DB      05fH            , 01H           ; 2776:  tkLParen->2779
        DB      01H                             ; 2778:  Reject
        DB      031H            , 01H           ; 2779:  Exp->2782
        DB      01H                             ; 2781:  Reject
        DB      060H            , 0FFH          ; 2782:  tkRParen->Accept
        DB      01H                             ; 2784:  Reject

        DB      05fH            , 01H           ; 2785:  tkLParen->2788
        DB      01H                             ; 2787:  Reject
        DB      031H            , 01H           ; 2788:  Exp->2791
        DB      01H                             ; 2790:  Reject
        DB      01bH            , 01H           ; 2791:  optCommaExp->2794
        DB      01H                             ; 2793:  Reject
        DB      060H            , 0FFH          ; 2794:  tkRParen->Accept
        DB      01H                             ; 2796:  Reject

        DB      05fH            , 01H           ; 2797:  tkLParen->2800
        DB      01H                             ; 2799:  Reject
        DB      013H            , 01H           ; 2800:  expCommaExp->2803
        DB      01H                             ; 2802:  Reject
        DB      060H            , 0FFH          ; 2803:  tkRParen->Accept
        DB      01H                             ; 2805:  Reject

        DB      05fH            , 01H           ; 2806:  tkLParen->2809
        DB      01H                             ; 2808:  Reject
        DB      031H            , 01H           ; 2809:  Exp->2812
        DB      01H                             ; 2811:  Reject
        DB      012H            , 01H           ; 2812:  exp12->2815
        DB      01H                             ; 2814:  Reject
        DB      060H            , 0FFH          ; 2815:  tkRParen->Accept
        DB      01H                             ; 2817:  Reject

        DB      05fH            , 01H           ; 2818:  tkLParen->2821
        DB      01H                             ; 2820:  Reject
        DB      039H            , 01H           ; 2821:  IdArray->2824
        DB      01H                             ; 2823:  Reject
        DB      01bH            , 01H           ; 2824:  optCommaExp->2827
        DB      01H                             ; 2826:  Reject
        DB      060H            , 0FFH          ; 2827:  tkRParen->Accept
        DB      01H                             ; 2829:  Reject

        DB      056H            , 01H           ; 2830:  tkLbs->2833
        DB      01H                             ; 2832:  Reject
        DB      031H            , 01H           ; 2833:  Exp->2836
        DB      01H                             ; 2835:  Reject
        DB      03H                             ; 2836:  EMIT(opLbs)
        DW      opLbs                           
        DB      03H                             ; 2839:  EMIT(opChanOut)
        DW      opChanOut                       
        DB      063H            , 0FFH          ; 2842:  tkComma->Accept
        DB      01H                             ; 2844:  Reject

        DB      056H            , 01H           ; 2845:  tkLbs->2848
        DB      01H                             ; 2847:  Reject
        DB      031H            , 01H           ; 2848:  Exp->2851
        DB      01H                             ; 2850:  Reject
        DB      03H                             ; 2851:  EMIT(opLbs)
        DW      opLbs                           
        DB      03H                             ; 2854:  EMIT(opInputChan)
        DW      opInputChan                     
        DB      063H            , 0FFH          ; 2857:  tkComma->Accept
        DB      01H                             ; 2859:  Reject

        DB      09H             , 0FFH          ; 2860:  commaExp->Accept
        DB      00H                             ; 2862:  Accept

        DB      056H            , 03H           ; 2863:  tkLbs->2868
        DB      031H            , 0FFH          ; 2865:  Exp->Accept
        DB      01H                             ; 2867:  Reject
        DB      031H            , 01H           ; 2868:  Exp->2871
        DB      01H                             ; 2870:  Reject
        DB      03H                             ; 2871:  EMIT(opLbs)
        DW      opLbs                           
        DB      00H                             ; 2874:  Accept

        DB      05fH            , 01H           ; 2875:  tkLParen->2878
        DB      00H                             ; 2877:  Accept
        DB      02H             , 06H           ; 2878:  MARK(6)
        DB      041H            , 02H           ; 2880:  IdParm->2884
        DB      04H             , 02H           ; 2882:  empty->2886
        DB      063H            , 03H           ; 2884:  tkComma->2889
        DB      060H            , 0FFH          ; 2886:  tkRParen->Accept
        DB      01H                             ; 2888:  Reject
        DB      041H            , 0d9H          ; 2889:  IdParm->2884
        DB      01H                             ; 2891:  Reject

        DB      05fH            , 01H           ; 2892:  tkLParen->2895
        DB      00H                             ; 2894:  Accept
        DB      02H             , 06H           ; 2895:  MARK(6)
        DB      041H            , 01H           ; 2897:  IdParm->2900
        DB      01H                             ; 2899:  Reject
        DB      063H            , 03H           ; 2900:  tkComma->2905
        DB      060H            , 0FFH          ; 2902:  tkRParen->Accept
        DB      01H                             ; 2904:  Reject
        DB      041H            , 0d9H          ; 2905:  IdParm->2900
        DB      01H                             ; 2907:  Reject

        DB      02eH            , 0FFH          ; 2908:  EndPrint->Accept
        DB      0e0H, 04eH      , 028H          ; 2910:  tkTAB->2953
        DB      0e0H, 041H      , 01eH          ; 2913:  tkSPC->2946
        DB      063H            , 018H          ; 2916:  tkComma->2942
        DB      067H            , 012H          ; 2918:  tkSColon->2938
        DB      031H            , 01H           ; 2920:  Exp->2923
        DB      01H                             ; 2922:  Reject
        DB      063H            , 09H           ; 2923:  tkComma->2934
        DB      067H            , 03H           ; 2925:  tkSColon->2930
        DB      02fH            , 0FFH          ; 2927:  EndPrintExp->Accept
        DB      01H                             ; 2929:  Reject
        DB      03H                             ; 2930:  EMIT(opPrintItemSemi)
        DW      opPrintItemSemi                 
        DB      00H                             ; 2933:  Accept
        DB      03H                             ; 2934:  EMIT(opPrintItemComma)
        DW      opPrintItemComma                
        DB      00H                             ; 2937:  Accept
        DB      03H                             ; 2938:  EMIT(opPrintSemi)
        DW      opPrintSemi                     
        DB      00H                             ; 2941:  Accept
        DB      03H                             ; 2942:  EMIT(opPrintComma)
        DW      opPrintComma                    
        DB      00H                             ; 2945:  Accept
        DB      014H            , 01H           ; 2946:  fn1arg->2949
        DB      01H                             ; 2948:  Reject
        DB      03H                             ; 2949:  EMIT(opPrintSpc)
        DW      opPrintSpc                      
        DB      00H                             ; 2952:  Accept
        DB      014H            , 01H           ; 2953:  fn1arg->2956
        DB      01H                             ; 2955:  Reject
        DB      03H                             ; 2956:  EMIT(opPrintTab)
        DW      opPrintTab                      
        DB      00H                             ; 2959:  Accept

        DB      01fH            , 0deH          ; 2960:  printItem->2960
        DB      0e0H, 05cH      , 01H           ; 2962:  tkUSING->2966
        DB      00H                             ; 2965:  Accept
        DB      031H            , 01H           ; 2966:  Exp->2969
        DB      01H                             ; 2968:  Reject
        DB      03H                             ; 2969:  EMIT(opUsing)
        DW      opUsing                         
        DB      067H            , 01H           ; 2972:  tkSColon->2975
        DB      01H                             ; 2974:  Reject
        DB      021H            , 0deH          ; 2975:  printUsingItem->2975
        DB      00H                             ; 2977:  Accept

        DB      02eH            , 0FFH          ; 2978:  EndPrint->Accept
        DB      0e0H, 04eH      , 019H          ; 2980:  tkTAB->3008
        DB      0e0H, 041H      , 0eH           ; 2983:  tkSPC->3000
        DB      031H            , 01H           ; 2986:  Exp->2989
        DB      01H                             ; 2988:  Reject
        DB      063H            , 05H           ; 2989:  tkComma->2996
        DB      067H            , 03H           ; 2991:  tkSColon->2996
        DB      02fH            , 0FFH          ; 2993:  EndPrintExp->Accept
        DB      01H                             ; 2995:  Reject
        DB      03H                             ; 2996:  EMIT(opPrintItemSemi)
        DW      opPrintItemSemi                 
        DB      00H                             ; 2999:  Accept
        DB      014H            , 01H           ; 3000:  fn1arg->3003
        DB      01H                             ; 3002:  Reject
        DB      03H                             ; 3003:  EMIT(opPrintSpc)
        DW      opPrintSpc                      
        DB      04H             , 06H           ; 3006:  empty->3014
        DB      014H            , 01H           ; 3008:  fn1arg->3011
        DB      01H                             ; 3010:  Reject
        DB      03H                             ; 3011:  EMIT(opPrintTab)
        DW      opPrintTab                      
        DB      067H            , 0FFH          ; 3014:  tkSColon->Accept
        DB      063H            , 0FFH          ; 3016:  tkComma->Accept
        DB      00H                             ; 3018:  Accept

; state table = 3019 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
