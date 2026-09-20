        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 18:45:08 2026


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
        EXTRN   NtEndPrint:NEAR
        EXTRN   NtEndPrintExp:NEAR
        EXTRN   NtExp:NEAR
        EXTRN   NtIdArray:NEAR
PUBLIC  tIntNtDisp
PUBLIC  tExtNtDisp
PUBLIC  tExtNtHelp
PUBLIC  tState


;Internal Nonterminal Dispatch Table
tIntNtDisp      LABEL   WORD
        DW      80      ; commaExp
        DW      86      ; exp12
        DW      92      ; expCommaExp
        DW      101     ; fn1arg
        DW      110     ; fn12arg
        DW      122     ; fnBoundArg
        DW      134     ; lbsExpComma
        DW      149     ; optCommaExp
        DW      152     ; optFilenum
        DW      164     ; printItem
        DW      214     ; printList
        DW      231     ; printUsingItem


;External Nonterminal Dispatch Table
tExtNtDisp      LABEL   WORD
        DW      NtEndPrint
        DW      NtEndPrintExp
        DW      NtExp
        DW      NtIdArray

;External Nonterminal Help Text Table
tExtNtHelp      LABEL   BYTE
        DW      0       ; EndPrint
        DW      0       ; EndPrintExp
        DW      MSG_ExpExp
        DW      MSG_ExpVar

;Recursive Descent Parse State Tables
tState  LABEL   BYTE
        DB      013H            , 01H           ; 0:  Exp->3
        DB      01H                             ; 2:  Reject
        DB      0cH             , 0FFH          ; 3:  optCommaExp->Accept
        DB      01H                             ; 5:  Reject

        DB      07H             , 01H           ; 6:  expCommaExp->9
        DB      01H                             ; 8:  Reject
        DB      05H             , 01H           ; 9:  commaExp->12
        DB      01H                             ; 11:  Reject
        DB      03H                             ; 12:  EMIT(opStBsave)
        DW      opStBsave                       
        DB      00H                             ; 15:  Accept

        DB      013H            , 01H           ; 16:  Exp->19
        DB      01H                             ; 18:  Reject
        DB      06H             , 0FFH          ; 19:  exp12->Accept
        DB      01H                             ; 21:  Reject

        DB      0bH             , 00H           ; 22:  lbsExpComma->24
        DB      0fH             , 0FFH          ; 24:  printList->Accept
        DB      01H                             ; 26:  Reject

        DB      03H                             ; 27:  EMIT(opStWrite)
        DW      opStWrite                       
        DB      0bH             , 00H           ; 30:  lbsExpComma->32
        DB      0fH             , 0FFH          ; 32:  printList->Accept
        DB      01H                             ; 34:  Reject

        DB      08H             , 01H           ; 35:  fn1arg->38
        DB      01H                             ; 37:  Reject
        DB      03H                             ; 38:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 41:  Accept

        DB      09H             , 0FFH          ; 42:  fn12arg->Accept
        DB      01H                             ; 44:  Reject

        DB      0aH             , 0FFH          ; 45:  fnBoundArg->Accept
        DB      01H                             ; 47:  Reject

        DB      0aH             , 0FFH          ; 48:  fnBoundArg->Accept
        DB      01H                             ; 50:  Reject

        DB      016H            , 01H           ; 51:  tkLParen->54
        DB      01H                             ; 53:  Reject
        DB      013H            , 01H           ; 54:  Exp->57
        DB      01H                             ; 56:  Reject
        DB      018H            , 02H           ; 57:  tkComma->61
        DB      04H             , 03H           ; 59:  empty->64
        DB      0dH             , 01H           ; 61:  optFilenum->64
        DB      01H                             ; 63:  Reject
        DB      017H            , 0FFH          ; 64:  tkRParen->Accept
        DB      01H                             ; 66:  Reject

        DB      016H            , 01H           ; 67:  tkLParen->70
        DB      01H                             ; 69:  Reject
        DB      0dH             , 01H           ; 70:  optFilenum->73
        DB      01H                             ; 72:  Reject
        DB      017H            , 01H           ; 73:  tkRParen->76
        DB      01H                             ; 75:  Reject
        DB      03H                             ; 76:  EMIT(opFnIoctl_)
        DW      opFnIoctl_                      
        DB      00H                             ; 79:  Accept

        DB      018H            , 01H           ; 80:  tkComma->83
        DB      01H                             ; 82:  Reject
        DB      013H            , 0FFH          ; 83:  Exp->Accept
        DB      01H                             ; 85:  Reject

        DB      05H             , 01H           ; 86:  commaExp->89
        DB      01H                             ; 88:  Reject
        DB      0cH             , 0FFH          ; 89:  optCommaExp->Accept
        DB      01H                             ; 91:  Reject

        DB      013H            , 01H           ; 92:  Exp->95
        DB      01H                             ; 94:  Reject
        DB      018H            , 01H           ; 95:  tkComma->98
        DB      01H                             ; 97:  Reject
        DB      013H            , 0FFH          ; 98:  Exp->Accept
        DB      01H                             ; 100:  Reject

        DB      016H            , 01H           ; 101:  tkLParen->104
        DB      01H                             ; 103:  Reject
        DB      013H            , 01H           ; 104:  Exp->107
        DB      01H                             ; 106:  Reject
        DB      017H            , 0FFH          ; 107:  tkRParen->Accept
        DB      01H                             ; 109:  Reject

        DB      016H            , 01H           ; 110:  tkLParen->113
        DB      01H                             ; 112:  Reject
        DB      013H            , 01H           ; 113:  Exp->116
        DB      01H                             ; 115:  Reject
        DB      0cH             , 01H           ; 116:  optCommaExp->119
        DB      01H                             ; 118:  Reject
        DB      017H            , 0FFH          ; 119:  tkRParen->Accept
        DB      01H                             ; 121:  Reject

        DB      016H            , 01H           ; 122:  tkLParen->125
        DB      01H                             ; 124:  Reject
        DB      014H            , 01H           ; 125:  IdArray->128
        DB      01H                             ; 127:  Reject
        DB      0cH             , 01H           ; 128:  optCommaExp->131
        DB      01H                             ; 130:  Reject
        DB      017H            , 0FFH          ; 131:  tkRParen->Accept
        DB      01H                             ; 133:  Reject

        DB      015H            , 01H           ; 134:  tkLbs->137
        DB      01H                             ; 136:  Reject
        DB      013H            , 01H           ; 137:  Exp->140
        DB      01H                             ; 139:  Reject
        DB      03H                             ; 140:  EMIT(opLbs)
        DW      opLbs                           
        DB      03H                             ; 143:  EMIT(opChanOut)
        DW      opChanOut                       
        DB      018H            , 0FFH          ; 146:  tkComma->Accept
        DB      01H                             ; 148:  Reject

        DB      05H             , 0FFH          ; 149:  commaExp->Accept
        DB      00H                             ; 151:  Accept

        DB      015H            , 03H           ; 152:  tkLbs->157
        DB      013H            , 0FFH          ; 154:  Exp->Accept
        DB      01H                             ; 156:  Reject
        DB      013H            , 01H           ; 157:  Exp->160
        DB      01H                             ; 159:  Reject
        DB      03H                             ; 160:  EMIT(opLbs)
        DW      opLbs                           
        DB      00H                             ; 163:  Accept

        DB      011H            , 0FFH          ; 164:  EndPrint->Accept
        DB      023H            , 027H          ; 166:  tkTAB->207
        DB      022H            , 01eH          ; 168:  tkSPC->200
        DB      018H            , 018H          ; 170:  tkComma->196
        DB      019H            , 012H          ; 172:  tkSColon->192
        DB      013H            , 01H           ; 174:  Exp->177
        DB      01H                             ; 176:  Reject
        DB      018H            , 09H           ; 177:  tkComma->188
        DB      019H            , 03H           ; 179:  tkSColon->184
        DB      012H            , 0FFH          ; 181:  EndPrintExp->Accept
        DB      01H                             ; 183:  Reject
        DB      03H                             ; 184:  EMIT(opPrintItemSemi)
        DW      opPrintItemSemi                 
        DB      00H                             ; 187:  Accept
        DB      03H                             ; 188:  EMIT(opPrintItemComma)
        DW      opPrintItemComma                
        DB      00H                             ; 191:  Accept
        DB      03H                             ; 192:  EMIT(opPrintSemi)
        DW      opPrintSemi                     
        DB      00H                             ; 195:  Accept
        DB      03H                             ; 196:  EMIT(opPrintComma)
        DW      opPrintComma                    
        DB      00H                             ; 199:  Accept
        DB      08H             , 01H           ; 200:  fn1arg->203
        DB      01H                             ; 202:  Reject
        DB      03H                             ; 203:  EMIT(opPrintSpc)
        DW      opPrintSpc                      
        DB      00H                             ; 206:  Accept
        DB      08H             , 01H           ; 207:  fn1arg->210
        DB      01H                             ; 209:  Reject
        DB      03H                             ; 210:  EMIT(opPrintTab)
        DW      opPrintTab                      
        DB      00H                             ; 213:  Accept

        DB      0eH             , 0deH          ; 214:  printItem->214
        DB      025H            , 01H           ; 216:  tkUSING->219
        DB      00H                             ; 218:  Accept
        DB      013H            , 01H           ; 219:  Exp->222
        DB      01H                             ; 221:  Reject
        DB      03H                             ; 222:  EMIT(opUsing)
        DW      opUsing                         
        DB      019H            , 01H           ; 225:  tkSColon->228
        DB      01H                             ; 227:  Reject
        DB      010H            , 0deH          ; 228:  printUsingItem->228
        DB      00H                             ; 230:  Accept

        DB      011H            , 0FFH          ; 231:  EndPrint->Accept
        DB      023H            , 018H          ; 233:  tkTAB->259
        DB      022H            , 0eH           ; 235:  tkSPC->251
        DB      013H            , 01H           ; 237:  Exp->240
        DB      01H                             ; 239:  Reject
        DB      018H            , 05H           ; 240:  tkComma->247
        DB      019H            , 03H           ; 242:  tkSColon->247
        DB      012H            , 0FFH          ; 244:  EndPrintExp->Accept
        DB      01H                             ; 246:  Reject
        DB      03H                             ; 247:  EMIT(opPrintItemSemi)
        DW      opPrintItemSemi                 
        DB      00H                             ; 250:  Accept
        DB      08H             , 01H           ; 251:  fn1arg->254
        DB      01H                             ; 253:  Reject
        DB      03H                             ; 254:  EMIT(opPrintSpc)
        DW      opPrintSpc                      
        DB      04H             , 06H           ; 257:  empty->265
        DB      08H             , 01H           ; 259:  fn1arg->262
        DB      01H                             ; 261:  Reject
        DB      03H                             ; 262:  EMIT(opPrintTab)
        DW      opPrintTab                      
        DB      019H            , 0FFH          ; 265:  tkSColon->Accept
        DB      018H            , 0FFH          ; 267:  tkComma->Accept
        DB      00H                             ; 269:  Accept

; state table = 270 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
