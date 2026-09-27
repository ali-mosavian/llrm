        page    ,132
        TITLE   prsrwt - Parser reserved word table

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 10:03:51 2026


        .xlist
include version.inc
PRSRWT_ASM = ON
        IncludeOnce parser
        IncludeOnce psint
        IncludeOnce opcodes
        IncludeOnce prsorw
.list

assumes DS,DATA
assumes SS,DATA
assumes ES,NOTHING

sBegin  CP
assumes CS,CP
        EXTRN   Cg1or2Args:NEAR
        EXTRN   CgCall:NEAR
        EXTRN   CgCall:NEAR
        EXTRN   CgCircle:NEAR
        EXTRN   CgStmtCnt:NEAR
        EXTRN   CgStmtCnt:NEAR
        EXTRN   CgStmtCnt:NEAR
        EXTRN   CgDeclare:NEAR
        EXTRN   AmDEF:NEAR
        EXTRN   CgDeclare:NEAR
        EXTRN   Cg0or1Args:NEAR
        EXTRN   CgStmtCnt:NEAR
        EXTRN   Cg0or1Args:NEAR
        EXTRN   CgDeclare:NEAR
        EXTRN   AmGET:NEAR
        EXTRN   CgInput:NEAR
        EXTRN   Cg1or2Args:NEAR
        EXTRN   Cg2or3Args:NEAR
        EXTRN   Cg1or2Args:NEAR
        EXTRN   AmLINE:NEAR
        EXTRN   CgLineStmt:NEAR
        EXTRN   CgInput:NEAR
        EXTRN   CgStmtCnt:NEAR
        EXTRN   CgLock:NEAR
        EXTRN   CgMoveOpsToEnd:NEAR
        EXTRN   Cg2or3Args:NEAR
        EXTRN   Cg3or4Args:NEAR
        EXTRN   CgOn:NEAR
        EXTRN   CgOpen:NEAR
        EXTRN   AmPLAY:NEAR
        EXTRN   Cg1or2Args:NEAR
        EXTRN   Cg2or3Args:NEAR
        EXTRN   Cg2or3Args:NEAR
        EXTRN   AmPUT:NEAR
        EXTRN   CgInsert0or1:NEAR
        EXTRN   CgInsert0or1:NEAR
        EXTRN   CgInsert0or1:NEAR
        EXTRN   Cg0or1Args:NEAR
        EXTRN   CgMoveOpsToEnd:NEAR
        EXTRN   CgRun:NEAR
        EXTRN   Cg2or3Args:NEAR
        EXTRN   CgStmtCnt:NEAR
        EXTRN   Cg0or1Args:NEAR
        EXTRN   CgDeclare:NEAR
        EXTRN   Cg1or2Args:NEAR
        EXTRN   CgLock:NEAR
        EXTRN   Cg2or3Args:NEAR

;IRW -> equivalent ASCII Character Table
PUBLIC  mpIRWtoChar
mpIRWtoChar     LABEL   BYTE
        DB      '%'             ; IRW_EtInteger
        DB      '&'             ; IRW_EtLong
        DB      '!'             ; IRW_EtSingle
        DB      '#'             ; IRW_Lbs
        DB      '$'             ; IRW_EtString
        DB      '"'             ; IRW_DQuote
        DB      '\'             ; IRW_Idiv
        DB      0aH             ; IRW_NewLine
        DB      09H             ; IRW_HTab
        DB      '^'             ; IRW_Pwr
        DB      '_'             ; IRW_UScore
        DB      027H            ; IRW_SQuote
        DB      '('             ; IRW_LParen
        DB      ')'             ; IRW_RParen
        DB      '*'             ; IRW_Mult
        DB      '+'             ; IRW_Add
        DB      ','             ; IRW_Comma
        DB      '-'             ; IRW_Minus
        DB      '/'             ; IRW_Div
        DB      ':'             ; IRW_Colon
        DB      ';'             ; IRW_SColon
        DB      '<'             ; IRW_LT
        DB      '='             ; IRW_EQ
        DB      '>'             ; IRW_GT
        DB      '?'             ; IRW_QMark


;Special Character -> Operator Table
PUBLIC  mpIRWtoIOP
mpIRWtoIOP      LABEL   BYTE
        DB      0FFH                    ; tkEtInteger is not an operator
        DB      0FFH                    ; tkEtLong is not an operator
        DB      0FFH                    ; tkEtSingle is not an operator
        DB      0FFH                    ; tkLbs is not an operator
        DB      0FFH                    ; tkEtString is not an operator
        DB      0FFH                    ; tkDQuote is not an operator
        DB      IOP_Idiv                ; operator id
        DB      0FFH                    ; tkNewLine is not an operator
        DB      0FFH                    ; tkHTab is not an operator
        DB      IOP_Pwr                 ; operator id
        DB      0FFH                    ; tkUScore is not an operator
        DB      0FFH                    ; tkSQuote is not an operator
        DB      IOP_LParen              ; operator id
        DB      IOP_RParen              ; operator id
        DB      IOP_Mult                ; operator id
        DB      IOP_Add                 ; operator id
        DB      0FFH                    ; tkComma is not an operator
        DB      IOP_Minus               ; operator id
        DB      IOP_Div                 ; operator id
        DB      0FFH                    ; tkColon is not an operator
        DB      0FFH                    ; tkSColon is not an operator
        DB      IOP_LT                  ; operator id
        DB      IOP_EQ                  ; operator id
        DB      IOP_GT                  ; operator id
        DB      0FFH                    ; tkQMark is not an operator

t41Rw   LABEL   BYTE
        DW      25                      ; res word index for 1st entry
      ; "ABS" - function 
        DB       023H, 'B', 'S', 040H
        DW      1934                    ;Func parse table offset
      ; "ACCESS" 
        DB       051H, 'C', 'C', 'E', 'S', 'S', 00H
      ; "ALIAS" 
        DB       041H, 'L', 'I', 'A', 'S', 00H
      ; "AND" - operator 
        DB       022H, 'N', 'D', 080H
        DB      IOP_AND                 ; operator id
      ; "ANY" 
        DB       021H, 'N', 'Y', 00H
      ; "APPEND" 
        DB       051H, 'P', 'P', 'E', 'N', 'D', 00H
      ; "AS" 
        DB       011H, 'S', 00H
      ; "ASC" - function 
        DB       023H, 'S', 'C', 040H
        DW      1941                    ;Func parse table offset
      ; "ATN" - function 
        DB       023H, 'T', 'N', 040H
        DW      1948                    ;Func parse table offset
        DB      0                       ; marks end of table

t42Rw   LABEL   BYTE
        DW      34                      ; res word index for 1st entry
      ; "BASE" 
        DB       031H, 'A', 'S', 'E', 00H
      ; "BEEP" - statement 
        DB       033H, 'E', 'E', 'P', 01H
        DW      0                       ;Stmt parse table offset
      ; "BINARY" 
        DB       051H, 'I', 'N', 'A', 'R', 'Y', 00H
      ; "BLOAD" - statement  (code gen.) 
        DB       047H, 'L', 'O', 'A', 'D', 09H
        DW      4                       ;Stmt parse table offset
        DW      Cg1or2Args              ;Stmt code generator
        DW      opStBload1              ;Stmt code generator arg
      ; "BSAVE" - statement 
        DB       043H, 'S', 'A', 'V', 'E', 01H
        DW      10                      ;Stmt parse table offset
      ; "BYVAL" 
        DB       041H, 'Y', 'V', 'A', 'L', 00H
        DB      0                       ; marks end of table

t43Rw   LABEL   BYTE
        DW      40                      ; res word index for 1st entry
      ; "CALL" - statement  (code gen.) 
        DB       037H, 'A', 'L', 'L', 09H
        DW      20                      ;Stmt parse table offset
        DW      CgCall                  ;Stmt code generator
        DW      opStCall                ;Stmt code generator arg
      ; "CALLS" - statement  (code gen.) 
        DB       047H, 'A', 'L', 'L', 'S', 09H
        DW      39                      ;Stmt parse table offset
        DW      CgCall                  ;Stmt code generator
        DW      opStCalls               ;Stmt code generator arg
      ; "CASE" - statement 
        DB       033H, 'A', 'S', 'E', 01H
        DW      58                      ;Stmt parse table offset
      ; "CDBL" - function 
        DB       033H, 'D', 'B', 'L', 040H
        DW      1955                    ;Func parse table offset
      ; "CDECL" 
        DB       041H, 'D', 'E', 'C', 'L', 00H
      ; "CHAIN" - statement 
        DB       043H, 'H', 'A', 'I', 'N', 01H
        DW      73                      ;Stmt parse table offset
      ; "CHDIR" - statement 
        DB       043H, 'H', 'D', 'I', 'R', 01H
        DW      80                      ;Stmt parse table offset
      ; "CHR$" - function 
        DB       023H, 'H', 'R', 044H
        DW      1962                    ;Func parse table offset
      ; "CINT" - function 
        DB       033H, 'I', 'N', 'T', 040H
        DW      1969                    ;Func parse table offset
      ; "CIRCLE" - statement  (code gen.) 
        DB       057H, 'I', 'R', 'C', 'L', 'E', 09H
        DW      87                      ;Stmt parse table offset
        DW      CgCircle                ;Stmt code generator
        DW      opStCircle              ;Stmt code generator arg
      ; "CLEAR" - statement  (code gen.) 
        DB       047H, 'L', 'E', 'A', 'R', 09H
        DW      130                     ;Stmt parse table offset
        DW      CgStmtCnt               ;Stmt code generator
        DW      opStClear               ;Stmt code generator arg
      ; "CLNG" - function 
        DB       033H, 'L', 'N', 'G', 040H
        DW      1976                    ;Func parse table offset
      ; "CLOSE" - statement  (code gen.) 
        DB       047H, 'L', 'O', 'S', 'E', 09H
        DW      133                     ;Stmt parse table offset
        DW      CgStmtCnt               ;Stmt code generator
        DW      opStClose               ;Stmt code generator arg
      ; "CLS" - statement 
        DB       023H, 'L', 'S', 01H
        DW      142                     ;Stmt parse table offset
      ; "COLOR" - statement  (code gen.) 
        DB       047H, 'O', 'L', 'O', 'R', 09H
        DW      151                     ;Stmt parse table offset
        DW      CgStmtCnt               ;Stmt code generator
        DW      opStColor               ;Stmt code generator arg
      ; "COM" - statement 
        DB       023H, 'O', 'M', 01H
        DW      154                     ;Stmt parse table offset
      ; "COMMAND$" - function 
        DB       063H, 'O', 'M', 'M', 'A', 'N', 'D', 044H
        DW      1983                    ;Func parse table offset
      ; "COMMON" - statement - illegal in direct mode 
        DB       053H, 'O', 'M', 'M', 'O', 'N', 021H
        DW      163                     ;Stmt parse table offset
      ; "CONST" - statement - illegal in direct mode 
        DB       043H, 'O', 'N', 'S', 'T', 021H
        DW      218                     ;Stmt parse table offset
      ; "COS" - function 
        DB       023H, 'O', 'S', 040H
        DW      1987                    ;Func parse table offset
      ; "CSNG" - function 
        DB       033H, 'S', 'N', 'G', 040H
        DW      1994                    ;Func parse table offset
      ; "CSRLIN" - function 
        DB       053H, 'S', 'R', 'L', 'I', 'N', 040H
        DW      2001                    ;Func parse table offset
      ; "CVD" - function 
        DB       023H, 'V', 'D', 040H
        DW      2005                    ;Func parse table offset
      ; "CVDMBF" - function 
        DB       053H, 'V', 'D', 'M', 'B', 'F', 040H
        DW      2012                    ;Func parse table offset
      ; "CVI" - function 
        DB       023H, 'V', 'I', 040H
        DW      2019                    ;Func parse table offset
      ; "CVL" - function 
        DB       023H, 'V', 'L', 040H
        DW      2026                    ;Func parse table offset
      ; "CVS" - function 
        DB       023H, 'V', 'S', 040H
        DW      2033                    ;Func parse table offset
      ; "CVSMBF" - function 
        DB       053H, 'V', 'S', 'M', 'B', 'F', 040H
        DW      2040                    ;Func parse table offset
        DB      0                       ; marks end of table

t44Rw   LABEL   BYTE
        DW      68                      ; res word index for 1st entry
      ; "DATA" - illegal in direct mode 
        DB       031H, 'A', 'T', 'A', 020H
      ; "DATE$" - statement - function 
        DB       035H, 'A', 'T', 'E', 045H
        DW      2047                    ;Func parse table offset
        DW      230                     ;Stmt parse table offset
      ; "DECLARE" - statement  (code gen.) - illegal in direct mode 
        DB       067H, 'E', 'C', 'L', 'A', 'R', 'E', 029H
        DW      240                     ;Stmt parse table offset
        DW      CgDeclare               ;Stmt code generator
        DW      opStDeclare             ;Stmt code generator arg
      ; "DEF" - statement  (code gen.) 
        DB       02fH, 'E', 'F', 0aH
        DW      AmDEF
        DW      257                     ;Stmt parse table offset
        DW      CgDeclare               ;Stmt code generator
        DW      opStDefFn               ;Stmt code generator arg
        DW      273                     ;Stmt parse table offset
        DW      Cg0or1Args              ;Stmt code generator
        DW      opStDefSeg0             ;Stmt code generator arg
      ; "DEFDBL" - statement - illegal in direct mode 
        DB       053H, 'E', 'F', 'D', 'B', 'L', 021H
        DW      292                     ;Stmt parse table offset
      ; "DEFINT" - statement - illegal in direct mode 
        DB       053H, 'E', 'F', 'I', 'N', 'T', 021H
        DW      283                     ;Stmt parse table offset
      ; "DEFLNG" - statement - illegal in direct mode 
        DB       053H, 'E', 'F', 'L', 'N', 'G', 021H
        DW      286                     ;Stmt parse table offset
      ; "DEFSNG" - statement - illegal in direct mode 
        DB       053H, 'E', 'F', 'S', 'N', 'G', 021H
        DW      289                     ;Stmt parse table offset
      ; "DEFSTR" - statement - illegal in direct mode 
        DB       053H, 'E', 'F', 'S', 'T', 'R', 021H
        DW      295                     ;Stmt parse table offset
      ; "DIM" - statement - illegal in direct mode 
        DB       023H, 'I', 'M', 021H
        DW      298                     ;Stmt parse table offset
      ; "DO" - statement 
        DB       013H, 'O', 01H
        DW      324                     ;Stmt parse table offset
      ; "DOUBLE" 
        DB       051H, 'O', 'U', 'B', 'L', 'E', 00H
      ; "DRAW" - statement 
        DB       033H, 'R', 'A', 'W', 01H
        DW      352                     ;Stmt parse table offset
        DB      0                       ; marks end of table

t45Rw   LABEL   BYTE
        DW      81                      ; res word index for 1st entry
      ; "ELSE" - statement 
        DB       033H, 'L', 'S', 'E', 01H
        DW      375                     ;Stmt parse table offset
      ; "ELSEIF" - statement - illegal in direct mode 
        DB       053H, 'L', 'S', 'E', 'I', 'F', 021H
        DW      359                     ;Stmt parse table offset
      ; "END" - statement 
        DB       023H, 'N', 'D', 01H
        DW      384                     ;Stmt parse table offset
      ; "ENDIF" - statement 
        DB       043H, 'N', 'D', 'I', 'F', 01H
        DW      436                     ;Stmt parse table offset
      ; "ENVIRON" - statement 
        DB       063H, 'N', 'V', 'I', 'R', 'O', 'N', 01H
        DW      442                     ;Stmt parse table offset
      ; "ENVIRON$" - function 
        DB       063H, 'N', 'V', 'I', 'R', 'O', 'N', 044H
        DW      2051                    ;Func parse table offset
      ; "EOF" - function 
        DB       023H, 'O', 'F', 040H
        DW      2058                    ;Func parse table offset
      ; "EQV" - operator 
        DB       022H, 'Q', 'V', 080H
        DB      IOP_EQV                 ; operator id
      ; "ERASE" - statement  (code gen.) 
        DB       047H, 'R', 'A', 'S', 'E', 09H
        DW      449                     ;Stmt parse table offset
        DW      CgStmtCnt               ;Stmt code generator
        DW      opStErase               ;Stmt code generator arg
      ; "ERDEV" - function 
        DB       043H, 'R', 'D', 'E', 'V', 040H
        DW      2065                    ;Func parse table offset
      ; "ERDEV$" - function 
        DB       043H, 'R', 'D', 'E', 'V', 044H
        DW      2069                    ;Func parse table offset
      ; "ERL" - function 
        DB       023H, 'R', 'L', 040H
        DW      2073                    ;Func parse table offset
      ; "ERR" - function 
        DB       023H, 'R', 'R', 040H
        DW      2077                    ;Func parse table offset
      ; "ERROR" - statement 
        DB       043H, 'R', 'R', 'O', 'R', 01H
        DW      458                     ;Stmt parse table offset
      ; "EXIT" - statement 
        DB       033H, 'X', 'I', 'T', 01H
        DW      465                     ;Stmt parse table offset
      ; "EXP" - function 
        DB       023H, 'X', 'P', 040H
        DW      2081                    ;Func parse table offset
        DB      0                       ; marks end of table

t46Rw   LABEL   BYTE
        DW      97                      ; res word index for 1st entry
      ; "FIELD" - statement 
        DB       043H, 'I', 'E', 'L', 'D', 01H
        DW      493                     ;Stmt parse table offset
      ; "FILEATTR" - function 
        DB       073H, 'I', 'L', 'E', 'A', 'T', 'T', 'R', 040H
        DW      2088                    ;Func parse table offset
      ; "FILES" - statement  (code gen.) 
        DB       047H, 'I', 'L', 'E', 'S', 09H
        DW      525                     ;Stmt parse table offset
        DW      Cg0or1Args              ;Stmt code generator
        DW      opStFiles0              ;Stmt code generator arg
      ; "FIX" - function 
        DB       023H, 'I', 'X', 040H
        DW      2095                    ;Func parse table offset
      ; "FOR" - statement 
        DB       023H, 'O', 'R', 01H
        DW      528                     ;Stmt parse table offset
      ; "FRE" - function 
        DB       023H, 'R', 'E', 040H
        DW      2102                    ;Func parse table offset
      ; "FREEFILE" - function 
        DB       073H, 'R', 'E', 'E', 'F', 'I', 'L', 'E', 040H
        DW      2109                    ;Func parse table offset
      ; "FUNCTION" - statement  (code gen.) - illegal in direct mode 
        DB       077H, 'U', 'N', 'C', 'T', 'I', 'O', 'N', 029H
        DW      564                     ;Stmt parse table offset
        DW      CgDeclare               ;Stmt code generator
        DW      opStFunction            ;Stmt code generator arg
        DB      0                       ; marks end of table

t47Rw   LABEL   BYTE
        DW      105                     ; res word index for 1st entry
      ; "GET" - statement  (code gen.) 
        DB       02fH, 'E', 'T', 0aH
        DW      AmGET
        DW      582                     ;Stmt parse table offset
        DW      0                       ;Stmt code generator
        DW      0                       ;Stmt code generator arg
        DW      620                     ;Stmt parse table offset
        DW      0                       ;Stmt code generator
        DW      0                       ;Stmt code generator arg
      ; "GOSUB" - statement 
        DB       043H, 'O', 'S', 'U', 'B', 01H
        DW      639                     ;Stmt parse table offset
      ; "GOTO" - statement 
        DB       033H, 'O', 'T', 'O', 01H
        DW      645                     ;Stmt parse table offset
        DB      0                       ; marks end of table

t48Rw   LABEL   BYTE
        DW      108                     ; res word index for 1st entry
      ; "HEX$" - function 
        DB       023H, 'E', 'X', 044H
        DW      2113                    ;Func parse table offset
        DB      0                       ; marks end of table

t49Rw   LABEL   BYTE
        DW      109                     ; res word index for 1st entry
      ; "IF" - statement 
        DB       013H, 'F', 01H
        DW      651                     ;Stmt parse table offset
      ; "IMP" - operator 
        DB       022H, 'M', 'P', 080H
        DB      IOP_IMP                 ; operator id
      ; "INKEY$" - function 
        DB       043H, 'N', 'K', 'E', 'Y', 044H
        DW      2120                    ;Func parse table offset
      ; "INP" - function 
        DB       023H, 'N', 'P', 040H
        DW      2124                    ;Func parse table offset
      ; "INPUT" - statement  (code gen.) 
        DB       047H, 'N', 'P', 'U', 'T', 09H
        DW      657                     ;Stmt parse table offset
        DW      CgInput                 ;Stmt code generator
        DW      opInputPrompt           ;Stmt code generator arg
      ; "INPUT$" - function  (code gen.) 
        DB       047H, 'N', 'P', 'U', 'T', 054H
        DW      2131                    ;Func parse table offset
        DW      Cg1or2Args              ;Func code generator
        DW      opFnInput_1             ;Func code generator arg
      ; "INSTR" - function  (code gen.) 
        DB       047H, 'N', 'S', 'T', 'R', 050H
        DW      2147                    ;Func parse table offset
        DW      Cg2or3Args              ;Func code generator
        DW      opFnInstr2              ;Func code generator arg
      ; "INT" - function 
        DB       023H, 'N', 'T', 040H
        DW      2150                    ;Func parse table offset
      ; "INTEGER" 
        DB       061H, 'N', 'T', 'E', 'G', 'E', 'R', 00H
      ; "IOCTL" - statement 
        DB       043H, 'O', 'C', 'T', 'L', 01H
        DW      717                     ;Stmt parse table offset
      ; "IOCTL$" - function 
        DB       043H, 'O', 'C', 'T', 'L', 044H
        DW      2157                    ;Func parse table offset
      ; "IS" 
        DB       011H, 'S', 00H
        DB      0                       ; marks end of table

t4aRw   LABEL   BYTE
        DW      121                     ; res word index for 1st entry
        DB      0                       ; marks end of table

t4bRw   LABEL   BYTE
        DW      121                     ; res word index for 1st entry
      ; "KEY" - statement 
        DB       023H, 'E', 'Y', 01H
        DW      727                     ;Stmt parse table offset
      ; "KILL" - statement 
        DB       033H, 'I', 'L', 'L', 01H
        DW      771                     ;Stmt parse table offset
        DB      0                       ; marks end of table

t4cRw   LABEL   BYTE
        DW      123                     ; res word index for 1st entry
      ; "LBOUND" - function  (code gen.) 
        DB       057H, 'B', 'O', 'U', 'N', 'D', 050H
        DW      2170                    ;Func parse table offset
        DW      Cg1or2Args              ;Func code generator
        DW      opFnLbound1             ;Func code generator arg
      ; "LCASE$" - function 
        DB       043H, 'C', 'A', 'S', 'E', 044H
        DW      2173                    ;Func parse table offset
      ; "LEFT$" - function 
        DB       033H, 'E', 'F', 'T', 044H
        DW      2180                    ;Func parse table offset
      ; "LEN" - function 
        DB       023H, 'E', 'N', 040H
        DW      2187                    ;Func parse table offset
      ; "LET" - statement 
        DB       023H, 'E', 'T', 01H
        DW      778                     ;Stmt parse table offset
      ; "LINE" - statement  (code gen.) 
        DB       03fH, 'I', 'N', 'E', 0aH
        DW      AmLINE
        DW      784                     ;Stmt parse table offset
        DW      CgLineStmt              ;Stmt code generator
        DW      opStLine                ;Stmt code generator arg
        DW      828                     ;Stmt parse table offset
        DW      CgInput                 ;Stmt code generator
        DW      opStLineInput           ;Stmt code generator arg
      ; "LIST" 
        DB       031H, 'I', 'S', 'T', 00H
      ; "LOC" - function 
        DB       023H, 'O', 'C', 040H
        DW      2196                    ;Func parse table offset
      ; "LOCAL" 
        DB       041H, 'O', 'C', 'A', 'L', 00H
      ; "LOCATE" - statement  (code gen.) 
        DB       057H, 'O', 'C', 'A', 'T', 'E', 09H
        DW      872                     ;Stmt parse table offset
        DW      CgStmtCnt               ;Stmt code generator
        DW      opStLocate              ;Stmt code generator arg
      ; "LOCK" - statement  (code gen.) 
        DB       037H, 'O', 'C', 'K', 09H
        DW      875                     ;Stmt parse table offset
        DW      CgLock                  ;Stmt code generator
        DW      opStLock                ;Stmt code generator arg
      ; "LOF" - function 
        DB       023H, 'O', 'F', 040H
        DW      2203                    ;Func parse table offset
      ; "LOG" - function 
        DB       023H, 'O', 'G', 040H
        DW      2210                    ;Func parse table offset
      ; "LONG" 
        DB       031H, 'O', 'N', 'G', 00H
      ; "LOOP" - statement 
        DB       033H, 'O', 'O', 'P', 01H
        DW      903                     ;Stmt parse table offset
      ; "LPOS" - function 
        DB       033H, 'P', 'O', 'S', 040H
        DW      2217                    ;Func parse table offset
      ; "LPRINT" - statement 
        DB       053H, 'P', 'R', 'I', 'N', 'T', 01H
        DW      931                     ;Stmt parse table offset
      ; "LSET" - statement  (code gen.) 
        DB       037H, 'S', 'E', 'T', 09H
        DW      937                     ;Stmt parse table offset
        DW      CgMoveOpsToEnd          ;Stmt code generator
        DW      opStLset                ;Stmt code generator arg
      ; "LTRIM$" - function 
        DB       043H, 'T', 'R', 'I', 'M', 044H
        DW      2224                    ;Func parse table offset
        DB      0                       ; marks end of table

t4dRw   LABEL   BYTE
        DW      142                     ; res word index for 1st entry
      ; "MID$" - statement  (code gen.) - function  (code gen.) 
        DB       02dH, 'I', 'D', 05dH
        DW      2231                    ;Func parse table offset
        DW      Cg2or3Args              ;Func code generator
        DW      opFnMid_2               ;Func code generator arg
        DW      950                     ;Stmt parse table offset
        DW      Cg3or4Args              ;Stmt code generator
        DW      opStMid_2               ;Stmt code generator arg
      ; "MKD$" - function 
        DB       023H, 'K', 'D', 044H
        DW      2234                    ;Func parse table offset
      ; "MKDIR" - statement 
        DB       043H, 'K', 'D', 'I', 'R', 01H
        DW      972                     ;Stmt parse table offset
      ; "MKDMBF$" - function 
        DB       053H, 'K', 'D', 'M', 'B', 'F', 044H
        DW      2241                    ;Func parse table offset
      ; "MKI$" - function 
        DB       023H, 'K', 'I', 044H
        DW      2248                    ;Func parse table offset
      ; "MKL$" - function 
        DB       023H, 'K', 'L', 044H
        DW      2255                    ;Func parse table offset
      ; "MKS$" - function 
        DB       023H, 'K', 'S', 044H
        DW      2262                    ;Func parse table offset
      ; "MKSMBF$" - function 
        DB       053H, 'K', 'S', 'M', 'B', 'F', 044H
        DW      2269                    ;Func parse table offset
      ; "MOD" - operator 
        DB       022H, 'O', 'D', 080H
        DB      IOP_MOD                 ; operator id
        DB      0                       ; marks end of table

t4eRw   LABEL   BYTE
        DW      151                     ; res word index for 1st entry
      ; "NAME" - statement 
        DB       033H, 'A', 'M', 'E', 01H
        DW      979                     ;Stmt parse table offset
      ; "NEXT" - statement 
        DB       033H, 'E', 'X', 'T', 01H
        DW      992                     ;Stmt parse table offset
      ; "NOT" - operator 
        DB       022H, 'O', 'T', 080H
        DB      IOP_NOT                 ; operator id
        DB      0                       ; marks end of table

t4fRw   LABEL   BYTE
        DW      154                     ; res word index for 1st entry
      ; "OCT$" - function 
        DB       023H, 'C', 'T', 044H
        DW      2276                    ;Func parse table offset
      ; "OFF" 
        DB       021H, 'F', 'F', 00H
      ; "ON" - statement  (code gen.) 
        DB       017H, 'N', 09H
        DW      1027                    ;Stmt parse table offset
        DW      CgOn                    ;Stmt code generator
        DW      0                       ;Stmt code generator arg
      ; "OPEN" - statement  (code gen.) 
        DB       037H, 'P', 'E', 'N', 09H
        DW      1088                    ;Stmt parse table offset
        DW      CgOpen                  ;Stmt code generator
        DW      opStOpen2               ;Stmt code generator arg
      ; "OPTION" - statement - illegal in direct mode 
        DB       053H, 'P', 'T', 'I', 'O', 'N', 021H
        DW      1210                    ;Stmt parse table offset
      ; "OR" - operator 
        DB       012H, 'R', 080H
        DB      IOP_OR                  ; operator id
      ; "OUT" - statement 
        DB       023H, 'U', 'T', 01H
        DW      1226                    ;Stmt parse table offset
      ; "OUTPUT" 
        DB       051H, 'U', 'T', 'P', 'U', 'T', 00H
        DB      0                       ; marks end of table

t50Rw   LABEL   BYTE
        DW      162                     ; res word index for 1st entry
      ; "PAINT" - statement 
        DB       043H, 'A', 'I', 'N', 'T', 01H
        DW      1233                    ;Stmt parse table offset
      ; "PALETTE" - statement 
        DB       063H, 'A', 'L', 'E', 'T', 'T', 'E', 01H
        DW      1252                    ;Stmt parse table offset
      ; "PCOPY" - statement 
        DB       043H, 'C', 'O', 'P', 'Y', 01H
        DW      1272                    ;Stmt parse table offset
      ; "PEEK" - function 
        DB       033H, 'E', 'E', 'K', 040H
        DW      2283                    ;Func parse table offset
      ; "PEN" - statement - function 
        DB       025H, 'E', 'N', 041H
        DW      2290                    ;Func parse table offset
        DW      1279                    ;Stmt parse table offset
      ; "PLAY" - statement  (code gen.) - function 
        DB       0FFH, 011H, 03H, 'L', 'A', 'Y', 04aH
        DW      2297                    ;Func parse table offset
        DW      AmPLAY
        DW      1285                    ;Stmt parse table offset
        DW      0                       ;Stmt code generator
        DW      0                       ;Stmt code generator arg
        DW      1292                    ;Stmt parse table offset
        DW      0                       ;Stmt code generator
        DW      0                       ;Stmt code generator arg
      ; "PMAP" - function 
        DB       033H, 'M', 'A', 'P', 040H
        DW      2304                    ;Func parse table offset
      ; "POINT" - function  (code gen.) 
        DB       047H, 'O', 'I', 'N', 'T', 050H
        DW      2311                    ;Func parse table offset
        DW      Cg1or2Args              ;Func code generator
        DW      opFnPoint1              ;Func code generator arg
      ; "POKE" - statement 
        DB       033H, 'O', 'K', 'E', 01H
        DW      1298                    ;Stmt parse table offset
      ; "POS" - function 
        DB       023H, 'O', 'S', 040H
        DW      2314                    ;Func parse table offset
      ; "PRESET" - statement  (code gen.) 
        DB       057H, 'R', 'E', 'S', 'E', 'T', 09H
        DW      1305                    ;Stmt parse table offset
        DW      Cg2or3Args              ;Stmt code generator
        DW      opStPreset              ;Stmt code generator arg
      ; "PRINT" - statement 
        DB       043H, 'R', 'I', 'N', 'T', 01H
        DW      1311                    ;Stmt parse table offset
      ; "PSET" - statement  (code gen.) 
        DB       037H, 'S', 'E', 'T', 09H
        DW      1316                    ;Stmt parse table offset
        DW      Cg2or3Args              ;Stmt code generator
        DW      opStPset                ;Stmt code generator arg
      ; "PUT" - statement  (code gen.) 
        DB       02fH, 'U', 'T', 0aH
        DW      AmPUT
        DW      1322                    ;Stmt parse table offset
        DW      0                       ;Stmt code generator
        DW      0                       ;Stmt code generator arg
        DW      1360                    ;Stmt parse table offset
        DW      0                       ;Stmt code generator
        DW      0                       ;Stmt code generator arg
        DB      0                       ; marks end of table

t51Rw   LABEL   BYTE
        DW      176                     ; res word index for 1st entry
        DB      0                       ; marks end of table

t52Rw   LABEL   BYTE
        DW      176                     ; res word index for 1st entry
      ; "RANDOM" 
        DB       051H, 'A', 'N', 'D', 'O', 'M', 00H
      ; "RANDOMIZE" - statement 
        DB       083H, 'A', 'N', 'D', 'O', 'M', 'I', 'Z', 'E', 01H
        DW      1412                    ;Stmt parse table offset
      ; "READ" - statement 
        DB       033H, 'E', 'A', 'D', 01H
        DW      1422                    ;Stmt parse table offset
      ; "REDIM" - statement - illegal in direct mode 
        DB       043H, 'E', 'D', 'I', 'M', 021H
        DW      1439                    ;Stmt parse table offset
      ; "REM" 
        DB       021H, 'E', 'M', 00H
      ; "RESET" - statement 
        DB       043H, 'E', 'S', 'E', 'T', 01H
        DW      1459                    ;Stmt parse table offset
      ; "RESTORE" - statement  (code gen.) 
        DB       067H, 'E', 'S', 'T', 'O', 'R', 'E', 09H
        DW      1463                    ;Stmt parse table offset
        DW      CgInsert0or1            ;Stmt code generator
        DW      opStRestore0            ;Stmt code generator arg
      ; "RESUME" - statement  (code gen.) 
        DB       057H, 'E', 'S', 'U', 'M', 'E', 09H
        DW      1471                    ;Stmt parse table offset
        DW      CgInsert0or1            ;Stmt code generator
        DW      opStResume0             ;Stmt code generator arg
      ; "RETURN" - statement  (code gen.) 
        DB       057H, 'E', 'T', 'U', 'R', 'N', 09H
        DW      1490                    ;Stmt parse table offset
        DW      CgInsert0or1            ;Stmt code generator
        DW      opStReturn0             ;Stmt code generator arg
      ; "RIGHT$" - function 
        DB       043H, 'I', 'G', 'H', 'T', 044H
        DW      2321                    ;Func parse table offset
      ; "RMDIR" - statement 
        DB       043H, 'M', 'D', 'I', 'R', 01H
        DW      1498                    ;Stmt parse table offset
      ; "RND" - function  (code gen.) 
        DB       027H, 'N', 'D', 050H
        DW      2328                    ;Func parse table offset
        DW      Cg0or1Args              ;Func code generator
        DW      opFnRnd                 ;Func code generator arg
      ; "RSET" - statement  (code gen.) 
        DB       037H, 'S', 'E', 'T', 09H
        DW      1505                    ;Stmt parse table offset
        DW      CgMoveOpsToEnd          ;Stmt code generator
        DW      opStRset                ;Stmt code generator arg
      ; "RTRIM$" - function 
        DB       043H, 'T', 'R', 'I', 'M', 044H
        DW      2331                    ;Func parse table offset
      ; "RUN" - statement  (code gen.) 
        DB       027H, 'U', 'N', 09H
        DW      1518                    ;Stmt parse table offset
        DW      CgRun                   ;Stmt code generator
        DW      0                       ;Stmt code generator arg
        DB      0                       ; marks end of table

t53Rw   LABEL   BYTE
        DW      191                     ; res word index for 1st entry
      ; "SADD" - function 
        DB       033H, 'A', 'D', 'D', 040H
        DW      2338                    ;Func parse table offset
      ; "SCREEN" - statement  (code gen.) - function  (code gen.) 
        DB       05dH, 'C', 'R', 'E', 'E', 'N', 059H
        DW      2351                    ;Func parse table offset
        DW      Cg2or3Args              ;Func code generator
        DW      opFnScreen2             ;Func code generator arg
        DW      1529                    ;Stmt parse table offset
        DW      CgStmtCnt               ;Stmt code generator
        DW      opStScreen              ;Stmt code generator arg
      ; "SEEK" - statement - function 
        DB       035H, 'E', 'E', 'K', 041H
        DW      2354                    ;Func parse table offset
        DW      1532                    ;Stmt parse table offset
      ; "SEG" 
        DB       021H, 'E', 'G', 00H
      ; "SELECT" - statement 
        DB       053H, 'E', 'L', 'E', 'C', 'T', 01H
        DW      1542                    ;Stmt parse table offset
      ; "SETMEM" - function 
        DB       053H, 'E', 'T', 'M', 'E', 'M', 040H
        DW      2361                    ;Func parse table offset
      ; "SGN" - function 
        DB       023H, 'G', 'N', 040H
        DW      2368                    ;Func parse table offset
      ; "SHARED" - statement - illegal in direct mode 
        DB       053H, 'H', 'A', 'R', 'E', 'D', 021H
        DW      1554                    ;Stmt parse table offset
      ; "SHELL" - statement  (code gen.) - function 
        DB       049H, 'H', 'E', 'L', 'L', 049H
        DW      2375                    ;Func parse table offset
        DW      1572                    ;Stmt parse table offset
        DW      Cg0or1Args              ;Stmt code generator
        DW      opStShell0              ;Stmt code generator arg
      ; "SIGNAL" - statement 
        DB       053H, 'I', 'G', 'N', 'A', 'L', 01H
        DW      1575                    ;Stmt parse table offset
      ; "SIN" - function 
        DB       023H, 'I', 'N', 040H
        DW      2382                    ;Func parse table offset
      ; "SINGLE" 
        DB       051H, 'I', 'N', 'G', 'L', 'E', 00H
      ; "SLEEP" - statement 
        DB       043H, 'L', 'E', 'E', 'P', 01H
        DW      1584                    ;Stmt parse table offset
      ; "SOUND" - statement 
        DB       043H, 'O', 'U', 'N', 'D', 01H
        DW      1594                    ;Stmt parse table offset
      ; "SPACE$" - function 
        DB       043H, 'P', 'A', 'C', 'E', 044H
        DW      2389                    ;Func parse table offset
      ; "SPC" 
        DB       021H, 'P', 'C', 00H
      ; "SQR" - function 
        DB       023H, 'Q', 'R', 040H
        DW      2396                    ;Func parse table offset
      ; "STATIC" - statement - illegal in direct mode 
        DB       053H, 'T', 'A', 'T', 'I', 'C', 021H
        DW      1601                    ;Stmt parse table offset
      ; "STEP" 
        DB       031H, 'T', 'E', 'P', 00H
      ; "STICK" - function 
        DB       043H, 'T', 'I', 'C', 'K', 040H
        DW      2403                    ;Func parse table offset
      ; "STOP" - statement 
        DB       033H, 'T', 'O', 'P', 01H
        DW      1619                    ;Stmt parse table offset
      ; "STR$" - function 
        DB       023H, 'T', 'R', 044H
        DW      2410                    ;Func parse table offset
      ; "STRIG" - statement - function 
        DB       045H, 'T', 'R', 'I', 'G', 041H
        DW      2417                    ;Func parse table offset
        DW      1626                    ;Stmt parse table offset
      ; "STRING" 
        DB       051H, 'T', 'R', 'I', 'N', 'G', 00H
      ; "STRING$" - function 
        DB       053H, 'T', 'R', 'I', 'N', 'G', 044H
        DW      2424                    ;Func parse table offset
      ; "SUB" - statement  (code gen.) - illegal in direct mode 
        DB       027H, 'U', 'B', 029H
        DW      1641                    ;Stmt parse table offset
        DW      CgDeclare               ;Stmt code generator
        DW      opStSub                 ;Stmt code generator arg
      ; "SWAP" - statement 
        DB       033H, 'W', 'A', 'P', 01H
        DW      1659                    ;Stmt parse table offset
      ; "SYSTEM" - statement 
        DB       053H, 'Y', 'S', 'T', 'E', 'M', 01H
        DW      1674                    ;Stmt parse table offset
        DB      0                       ; marks end of table

t54Rw   LABEL   BYTE
        DW      219                     ; res word index for 1st entry
      ; "TAB" 
        DB       021H, 'A', 'B', 00H
      ; "TAN" - function 
        DB       023H, 'A', 'N', 040H
        DW      2431                    ;Func parse table offset
      ; "THEN" 
        DB       031H, 'H', 'E', 'N', 00H
      ; "TIME$" - statement - function 
        DB       035H, 'I', 'M', 'E', 045H
        DW      2438                    ;Func parse table offset
        DW      1678                    ;Stmt parse table offset
      ; "TIMER" - statement - function 
        DB       045H, 'I', 'M', 'E', 'R', 041H
        DW      2442                    ;Func parse table offset
        DW      1688                    ;Stmt parse table offset
      ; "TO" 
        DB       011H, 'O', 00H
      ; "TROFF" - statement 
        DB       043H, 'R', 'O', 'F', 'F', 01H
        DW      1694                    ;Stmt parse table offset
      ; "TRON" - statement 
        DB       033H, 'R', 'O', 'N', 01H
        DW      1698                    ;Stmt parse table offset
      ; "TYPE" - statement - illegal in direct mode 
        DB       033H, 'Y', 'P', 'E', 021H
        DW      1702                    ;Stmt parse table offset
        DB      0                       ; marks end of table

t55Rw   LABEL   BYTE
        DW      228                     ; res word index for 1st entry
      ; "UBOUND" - function  (code gen.) 
        DB       057H, 'B', 'O', 'U', 'N', 'D', 050H
        DW      2446                    ;Func parse table offset
        DW      Cg1or2Args              ;Func code generator
        DW      opFnUbound1             ;Func code generator arg
      ; "UCASE$" - function 
        DB       043H, 'C', 'A', 'S', 'E', 044H
        DW      2449                    ;Func parse table offset
      ; "UEVENT" - statement 
        DB       053H, 'E', 'V', 'E', 'N', 'T', 01H
        DW      1739                    ;Stmt parse table offset
      ; "UNLOCK" - statement  (code gen.) 
        DB       057H, 'N', 'L', 'O', 'C', 'K', 09H
        DW      1711                    ;Stmt parse table offset
        DW      CgLock                  ;Stmt code generator
        DW      opStUnlock              ;Stmt code generator arg
      ; "UNTIL" 
        DB       041H, 'N', 'T', 'I', 'L', 00H
      ; "USING" 
        DB       041H, 'S', 'I', 'N', 'G', 00H
        DB      0                       ; marks end of table

t56Rw   LABEL   BYTE
        DW      234                     ; res word index for 1st entry
      ; "VAL" - function 
        DB       023H, 'A', 'L', 040H
        DW      2456                    ;Func parse table offset
      ; "VARPTR" - function 
        DB       053H, 'A', 'R', 'P', 'T', 'R', 040H
        DW      2463                    ;Func parse table offset
      ; "VARPTR$" - function 
        DB       053H, 'A', 'R', 'P', 'T', 'R', 044H
        DW      2476                    ;Func parse table offset
      ; "VARSEG" - function 
        DB       053H, 'A', 'R', 'S', 'E', 'G', 040H
        DW      2491                    ;Func parse table offset
      ; "VIEW" - statement 
        DB       033H, 'I', 'E', 'W', 01H
        DW      1745                    ;Stmt parse table offset
        DB      0                       ; marks end of table

t57Rw   LABEL   BYTE
        DW      239                     ; res word index for 1st entry
      ; "WAIT" - statement  (code gen.) 
        DB       037H, 'A', 'I', 'T', 09H
        DW      1809                    ;Stmt parse table offset
        DW      Cg2or3Args              ;Stmt code generator
        DW      opStWait2               ;Stmt code generator arg
      ; "WEND" - statement 
        DB       033H, 'E', 'N', 'D', 01H
        DW      1815                    ;Stmt parse table offset
      ; "WHILE" - statement 
        DB       043H, 'H', 'I', 'L', 'E', 01H
        DW      1821                    ;Stmt parse table offset
      ; "WIDTH" - statement 
        DB       043H, 'I', 'D', 'T', 'H', 01H
        DW      1830                    ;Stmt parse table offset
      ; "WINDOW" - statement 
        DB       053H, 'I', 'N', 'D', 'O', 'W', 01H
        DW      1877                    ;Stmt parse table offset
      ; "WRITE" - statement 
        DB       043H, 'R', 'I', 'T', 'E', 01H
        DW      1909                    ;Stmt parse table offset
        DB      0                       ; marks end of table

t58Rw   LABEL   BYTE
        DW      245                     ; res word index for 1st entry
      ; "XOR" - operator 
        DB       022H, 'O', 'R', 080H
        DB      IOP_XOR                 ; operator id
        DB      0                       ; marks end of table

t59Rw   LABEL   BYTE
        DW      246                     ; res word index for 1st entry
        DB      0                       ; marks end of table

;Table of pointers to reserved word Table for each letter
PUBLIC  tRw

tRw     LABEL   WORD
        DW      OFFSET CP:t41Rw
        DW      OFFSET CP:t42Rw
        DW      OFFSET CP:t43Rw
        DW      OFFSET CP:t44Rw
        DW      OFFSET CP:t45Rw
        DW      OFFSET CP:t46Rw
        DW      OFFSET CP:t47Rw
        DW      OFFSET CP:t48Rw
        DW      OFFSET CP:t49Rw
        DW      OFFSET CP:t4aRw
        DW      OFFSET CP:t4bRw
        DW      OFFSET CP:t4cRw
        DW      OFFSET CP:t4dRw
        DW      OFFSET CP:t4eRw
        DW      OFFSET CP:t4fRw
        DW      OFFSET CP:t50Rw
        DW      OFFSET CP:t51Rw
        DW      OFFSET CP:t52Rw
        DW      OFFSET CP:t53Rw
        DW      OFFSET CP:t54Rw
        DW      OFFSET CP:t55Rw
        DW      OFFSET CP:t56Rw
        DW      OFFSET CP:t57Rw
        DW      OFFSET CP:t58Rw
        DW      OFFSET CP:t59Rw
sEnd    CP

        END
