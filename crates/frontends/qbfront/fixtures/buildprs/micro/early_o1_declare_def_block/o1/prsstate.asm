        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 17:57:11 2026


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
        DW      417     ; caseItem
        DW      398     ; commaExp
        DW      376     ; evSwitch
        DW      395     ; expCommaExp
        DW      401     ; EMITFFFF
        DW      405     ; fn1arg
        DW      414     ; optCommaExp


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

        DB      0aH             , 01H           ; 151:  fn1arg->154
        DB      01H                             ; 153:  Reject
        DB      03H                             ; 154:  EMIT(opEvCom)
        DW      opEvCom                         
        DB      04H             , 0e1H, 06eH    ; 157:  empty->366

        DB      049H            , 011H          ; 160:  tkSHARED->179
        DB      03H                             ; 162:  EMIT(opStCommon)
        DW      opStCommon                      
        DB      09H             , 01H           ; 165:  EMITFFFF->168
        DB      01H                             ; 167:  Reject
        DB      023H            , 03H           ; 168:  tkDiv->173
        DB      09H             , 01eH          ; 170:  EMITFFFF->202
        DB      01H                             ; 172:  Reject
        DB      01cH            , 01H           ; 173:  IdNamCom->176
        DB      01H                             ; 175:  Reject
        DB      023H            , 018H          ; 176:  tkDiv->202
        DB      01H                             ; 178:  Reject
        DB      03H                             ; 179:  EMIT(opShared)
        DW      opShared                        
        DB      03H                             ; 182:  EMIT(opStCommon)
        DW      opStCommon                      
        DB      09H             , 01H           ; 185:  EMITFFFF->188
        DB      01H                             ; 187:  Reject
        DB      023H            , 03H           ; 188:  tkDiv->193
        DB      09H             , 07H           ; 190:  EMITFFFF->199
        DB      01H                             ; 192:  Reject
        DB      01cH            , 01H           ; 193:  IdNamCom->196
        DB      01H                             ; 195:  Reject
        DB      023H            , 01H           ; 196:  tkDiv->199
        DB      01H                             ; 198:  Reject
        DB      0dH             , 01H           ; 199:  ACTIONidShared->202
        DB      01H                             ; 201:  Reject
        DB      0cH             , 01H           ; 202:  ACTIONidCommon->205
        DB      01H                             ; 204:  Reject
        DB      018H            , 01H           ; 205:  IdAryI->208
        DB      01H                             ; 207:  Reject
        DB      026H            , 01H           ; 208:  tkComma->211
        DB      00H                             ; 210:  Accept
        DB      018H            , 0dbH          ; 211:  IdAryI->208
        DB      01H                             ; 213:  Reject

        DB      03H                             ; 214:  EMIT(opStConst)
        DW      opStConst                       
        DB      0fH             , 01H           ; 217:  ConstAssign->220
        DB      01H                             ; 219:  Reject
        DB      026H            , 01H           ; 220:  tkComma->223
        DB      00H                             ; 222:  Accept
        DB      0fH             , 0dbH          ; 223:  ConstAssign->220
        DB      01H                             ; 225:  Reject

        DB      022H            , 01H           ; 226:  tkEQ->229
        DB      01H                             ; 228:  Reject
        DB      016H            , 01H           ; 229:  Exp->232
        DB      01H                             ; 231:  Reject
        DB      03H                             ; 232:  EMIT(opStDate_)
        DW      opStDate_                       
        DB      00H                             ; 235:  Accept

        DB      043H            , 06H           ; 236:  tkFUNCTION->244
        DB      04bH            , 01H           ; 238:  tkSUB->241
        DB      01H                             ; 240:  Reject
        DB      01dH            , 04H           ; 241:  IdSubDecl->247
        DB      01H                             ; 243:  Reject
        DB      01bH            , 01H           ; 244:  IdFuncDecl->247
        DB      01H                             ; 246:  Reject
        DB      02H             , 03H           ; 247:  MARK(3)
        DB      021H            , 0FFH          ; 249:  parms->Accept
        DB      01H                             ; 251:  Reject

        DB      01aH            , 01H           ; 252:  IdFn->255
        DB      01H                             ; 254:  Reject
        DB      02H             , 03H           ; 255:  MARK(3)
        DB      021H            , 01H           ; 257:  parms->260
        DB      01H                             ; 259:  Reject
        DB      022H            , 01H           ; 260:  tkEQ->263
        DB      00H                             ; 262:  Accept
        DB      02H             , 05H           ; 263:  MARK(5)
        DB      04H             , 0e1H, 0a1H    ; 265:  empty->417

        DB      048H            , 01H           ; 268:  tkSEG->271
        DB      01H                             ; 270:  Reject
        DB      022H            , 0e1H, 0a1H    ; 271:  tkEQ->417
        DB      00H                             ; 274:  Accept

        DB      011H            , 0FFH          ; 275:  DeflistI2->Accept
        DB      01H                             ; 277:  Reject

        DB      012H            , 0FFH          ; 278:  DeflistI4->Accept
        DB      01H                             ; 280:  Reject

        DB      013H            , 0FFH          ; 281:  DeflistR4->Accept
        DB      01H                             ; 283:  Reject

        DB      014H            , 0FFH          ; 284:  DeflistR8->Accept
        DB      01H                             ; 286:  Reject

        DB      015H            , 0FFH          ; 287:  DeflistSD->Accept
        DB      01H                             ; 289:  Reject

        DB      049H            , 02H           ; 290:  tkSHARED->294
        DB      04H             , 06H           ; 292:  empty->300
        DB      0dH             , 01H           ; 294:  ACTIONidShared->297
        DB      01H                             ; 296:  Reject
        DB      03H                             ; 297:  EMIT(opShared)
        DW      opShared                        
        DB      03H                             ; 300:  EMIT(opStDim)
        DW      opStDim                         
        DB      09H             , 01H           ; 303:  EMITFFFF->306
        DB      01H                             ; 305:  Reject
        DB      017H            , 01H           ; 306:  IdAryDim->309
        DB      01H                             ; 308:  Reject
        DB      026H            , 01H           ; 309:  tkComma->312
        DB      00H                             ; 311:  Accept
        DB      017H            , 0dbH          ; 312:  IdAryDim->309
        DB      01H                             ; 314:  Reject

        DB      04fH            , 0fH           ; 315:  tkWHILE->332
        DB      04eH            , 04H           ; 317:  tkUNTIL->323
        DB      03H                             ; 319:  EMIT(opStDo)
        DW      opStDo                          
        DB      00H                             ; 322:  Accept
        DB      016H            , 01H           ; 323:  Exp->326
        DB      01H                             ; 325:  Reject
        DB      03H                             ; 326:  EMIT(opStDoUntil)
        DW      opStDoUntil                     
        DB      09H             , 0FFH          ; 329:  EMITFFFF->Accept
        DB      01H                             ; 331:  Reject
        DB      016H            , 01H           ; 332:  Exp->335
        DB      01H                             ; 334:  Reject
        DB      03H                             ; 335:  EMIT(opStDoWhile)
        DW      opStDoWhile                     
        DB      09H             , 0FFH          ; 338:  EMITFFFF->Accept
        DB      01H                             ; 340:  Reject

        DB      03H                             ; 341:  EMIT(opEvPen)
        DW      opEvPen                         
        DB      04H             , 014H          ; 344:  empty->366

        DB      016H            , 01H           ; 346:  Exp->349
        DB      01H                             ; 348:  Reject
        DB      03H                             ; 349:  EMIT(opStPlay)
        DW      opStPlay                        
        DB      00H                             ; 352:  Accept

        DB      03H                             ; 353:  EMIT(opEvPlay0)
        DW      opEvPlay0                       
        DB      04H             , 08H           ; 356:  empty->366

        DB      03H                             ; 358:  EMIT(opEvTimer0)
        DW      opEvTimer0                      
        DB      04H             , 03H           ; 361:  empty->366

        DB      03H                             ; 363:  EMIT(opEvUEvent)
        DW      opEvUEvent                      
        DB      07H             , 0FFH          ; 366:  evSwitch->Accept
        DB      01H                             ; 368:  Reject

        DB      016H            , 01H           ; 369:  Exp->372
        DB      01H                             ; 371:  Reject
        DB      03H                             ; 372:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 375:  Accept

        DB      045H            , 0dH           ; 376:  tkON->391
        DB      044H            , 07H           ; 378:  tkOFF->387
        DB      04aH            , 01H           ; 380:  tkSTOP->383
        DB      01H                             ; 382:  Reject
        DB      03H                             ; 383:  EMIT(opEvStop)
        DW      opEvStop                        
        DB      00H                             ; 386:  Accept
        DB      03H                             ; 387:  EMIT(opEvOff)
        DW      opEvOff                         
        DB      00H                             ; 390:  Accept
        DB      03H                             ; 391:  EMIT(opEvOn)
        DW      opEvOn                          
        DB      00H                             ; 394:  Accept

        DB      016H            , 01H           ; 395:  Exp->398
        DB      01H                             ; 397:  Reject

        DB      026H            , 011H          ; 398:  tkComma->417
        DB      01H                             ; 400:  Reject

        DB      03H                             ; 401:  EMIT(UNDEFINED)
        DW      UNDEFINED                       
        DB      00H                             ; 404:  Accept

        DB      024H            , 01H           ; 405:  tkLParen->408
        DB      01H                             ; 407:  Reject
        DB      016H            , 01H           ; 408:  Exp->411
        DB      01H                             ; 410:  Reject
        DB      025H            , 0FFH          ; 411:  tkRParen->Accept
        DB      01H                             ; 413:  Reject

        DB      026H            , 01H           ; 414:  tkComma->417
        DB      00H                             ; 416:  Accept
        DB      016H            , 0FFH          ; 417:  Exp->Accept
        DB      01H                             ; 419:  Reject

; state table = 420 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
