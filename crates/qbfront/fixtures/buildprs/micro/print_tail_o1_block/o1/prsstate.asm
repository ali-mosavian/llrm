        page    ,132
        TITLE   prsstate - Parser state tables

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 18:45:10 2026


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
        DW      75      ; commaExp
        DW      66      ; exp12
        DW      72      ; expCommaExp
        DW      78      ; fn1arg
        DW      84      ; fn12arg
        DW      90      ; fnBoundArg
        DW      102     ; lbsExpComma
        DW      117     ; optCommaExp
        DW      120     ; optFilenum
        DW      132     ; printItem
        DW      181     ; printList
        DW      198     ; printUsingItem


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
        DB      013H            , 043H          ; 0:  Exp->69
        DB      01H                             ; 2:  Reject

        DB      07H             , 01H           ; 3:  expCommaExp->6
        DB      01H                             ; 5:  Reject
        DB      05H             , 01H           ; 6:  commaExp->9
        DB      01H                             ; 8:  Reject
        DB      03H                             ; 9:  EMIT(opStBsave)
        DW      opStBsave                       
        DB      00H                             ; 12:  Accept

        DB      013H            , 01H           ; 13:  Exp->16
        DB      01H                             ; 15:  Reject
        DB      06H             , 0FFH          ; 16:  exp12->Accept
        DB      01H                             ; 18:  Reject

        DB      03H                             ; 19:  EMIT(opStWrite)
        DW      opStWrite                       

        DB      0bH             , 00H           ; 22:  lbsExpComma->24
        DB      0fH             , 0FFH          ; 24:  printList->Accept
        DB      01H                             ; 26:  Reject

        DB      08H             , 01H           ; 27:  fn1arg->30
        DB      01H                             ; 29:  Reject
        DB      03H                             ; 30:  EMIT(opFnAbs)
        DW      opFnAbs                         
        DB      00H                             ; 33:  Accept

        DB      09H             , 0FFH          ; 34:  fn12arg->Accept
        DB      01H                             ; 36:  Reject

        DB      0aH             , 0FFH          ; 37:  fnBoundArg->Accept
        DB      01H                             ; 39:  Reject

        DB      016H            , 01H           ; 40:  tkLParen->43
        DB      01H                             ; 42:  Reject
        DB      013H            , 01H           ; 43:  Exp->46
        DB      01H                             ; 45:  Reject
        DB      018H            , 02H           ; 46:  tkComma->50
        DB      04H             , 031H          ; 48:  empty->99
        DB      0dH             , 02fH          ; 50:  optFilenum->99
        DB      01H                             ; 52:  Reject

        DB      016H            , 01H           ; 53:  tkLParen->56
        DB      01H                             ; 55:  Reject
        DB      0dH             , 01H           ; 56:  optFilenum->59
        DB      01H                             ; 58:  Reject
        DB      017H            , 01H           ; 59:  tkRParen->62
        DB      01H                             ; 61:  Reject
        DB      03H                             ; 62:  EMIT(opFnIoctl_)
        DW      opFnIoctl_                      
        DB      00H                             ; 65:  Accept

        DB      05H             , 01H           ; 66:  commaExp->69
        DB      01H                             ; 68:  Reject
        DB      0cH             , 0FFH          ; 69:  optCommaExp->Accept
        DB      01H                             ; 71:  Reject

        DB      013H            , 01H           ; 72:  Exp->75
        DB      01H                             ; 74:  Reject

        DB      018H            , 02dH          ; 75:  tkComma->122
        DB      01H                             ; 77:  Reject

        DB      016H            , 01H           ; 78:  tkLParen->81
        DB      01H                             ; 80:  Reject
        DB      013H            , 010H          ; 81:  Exp->99
        DB      01H                             ; 83:  Reject

        DB      016H            , 01H           ; 84:  tkLParen->87
        DB      01H                             ; 86:  Reject
        DB      013H            , 07H           ; 87:  Exp->96
        DB      01H                             ; 89:  Reject

        DB      016H            , 01H           ; 90:  tkLParen->93
        DB      01H                             ; 92:  Reject
        DB      014H            , 01H           ; 93:  IdArray->96
        DB      01H                             ; 95:  Reject
        DB      0cH             , 01H           ; 96:  optCommaExp->99
        DB      01H                             ; 98:  Reject
        DB      017H            , 0FFH          ; 99:  tkRParen->Accept
        DB      01H                             ; 101:  Reject

        DB      015H            , 01H           ; 102:  tkLbs->105
        DB      01H                             ; 104:  Reject
        DB      013H            , 01H           ; 105:  Exp->108
        DB      01H                             ; 107:  Reject
        DB      03H                             ; 108:  EMIT(opLbs)
        DW      opLbs                           
        DB      03H                             ; 111:  EMIT(opChanOut)
        DW      opChanOut                       
        DB      018H            , 0FFH          ; 114:  tkComma->Accept
        DB      01H                             ; 116:  Reject

        DB      05H             , 0FFH          ; 117:  commaExp->Accept
        DB      00H                             ; 119:  Accept

        DB      015H            , 03H           ; 120:  tkLbs->125
        DB      013H            , 0FFH          ; 122:  Exp->Accept
        DB      01H                             ; 124:  Reject
        DB      013H            , 01H           ; 125:  Exp->128
        DB      01H                             ; 127:  Reject
        DB      03H                             ; 128:  EMIT(opLbs)
        DW      opLbs                           
        DB      00H                             ; 131:  Accept

        DB      011H            , 0FFH          ; 132:  EndPrint->Accept
        DB      023H            , 026H          ; 134:  tkTAB->174
        DB      022H            , 01dH          ; 136:  tkSPC->167
        DB      018H            , 017H          ; 138:  tkComma->163
        DB      019H            , 011H          ; 140:  tkSColon->159
        DB      013H            , 01H           ; 142:  Exp->145
        DB      01H                             ; 144:  Reject
        DB      018H            , 08H           ; 145:  tkComma->155
        DB      019H            , 02H           ; 147:  tkSColon->151
        DB      04H             , 03cH          ; 149:  empty->211
        DB      03H                             ; 151:  EMIT(opPrintItemSemi)
        DW      opPrintItemSemi                 
        DB      00H                             ; 154:  Accept
        DB      03H                             ; 155:  EMIT(opPrintItemComma)
        DW      opPrintItemComma                
        DB      00H                             ; 158:  Accept
        DB      03H                             ; 159:  EMIT(opPrintSemi)
        DW      opPrintSemi                     
        DB      00H                             ; 162:  Accept
        DB      03H                             ; 163:  EMIT(opPrintComma)
        DW      opPrintComma                    
        DB      00H                             ; 166:  Accept
        DB      08H             , 01H           ; 167:  fn1arg->170
        DB      01H                             ; 169:  Reject
        DB      03H                             ; 170:  EMIT(opPrintSpc)
        DW      opPrintSpc                      
        DB      00H                             ; 173:  Accept
        DB      08H             , 01H           ; 174:  fn1arg->177
        DB      01H                             ; 176:  Reject
        DB      03H                             ; 177:  EMIT(opPrintTab)
        DW      opPrintTab                      
        DB      00H                             ; 180:  Accept

        DB      0eH             , 0deH          ; 181:  printItem->181
        DB      025H            , 01H           ; 183:  tkUSING->186
        DB      00H                             ; 185:  Accept
        DB      013H            , 01H           ; 186:  Exp->189
        DB      01H                             ; 188:  Reject
        DB      03H                             ; 189:  EMIT(opUsing)
        DW      opUsing                         
        DB      019H            , 01H           ; 192:  tkSColon->195
        DB      01H                             ; 194:  Reject
        DB      010H            , 0deH          ; 195:  printUsingItem->195
        DB      00H                             ; 197:  Accept

        DB      011H            , 0FFH          ; 198:  EndPrint->Accept
        DB      023H            , 018H          ; 200:  tkTAB->226
        DB      022H            , 0eH           ; 202:  tkSPC->218
        DB      013H            , 01H           ; 204:  Exp->207
        DB      01H                             ; 206:  Reject
        DB      018H            , 05H           ; 207:  tkComma->214
        DB      019H            , 03H           ; 209:  tkSColon->214
        DB      012H            , 0FFH          ; 211:  EndPrintExp->Accept
        DB      01H                             ; 213:  Reject
        DB      03H                             ; 214:  EMIT(opPrintItemSemi)
        DW      opPrintItemSemi                 
        DB      00H                             ; 217:  Accept
        DB      08H             , 01H           ; 218:  fn1arg->221
        DB      01H                             ; 220:  Reject
        DB      03H                             ; 221:  EMIT(opPrintSpc)
        DW      opPrintSpc                      
        DB      04H             , 06H           ; 224:  empty->232
        DB      08H             , 01H           ; 226:  fn1arg->229
        DB      01H                             ; 228:  Reject
        DB      03H                             ; 229:  EMIT(opPrintTab)
        DW      opPrintTab                      
        DB      019H            , 0FFH          ; 232:  tkSColon->Accept
        DB      018H            , 0FFH          ; 234:  tkComma->Accept
        DB      00H                             ; 236:  Accept

; state table = 237 bytes

sEnd    CP

sBegin  DATA
sEnd    DATA

        END
