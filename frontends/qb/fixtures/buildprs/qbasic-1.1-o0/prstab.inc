;prstab.inc - Parser constants

;*********************************
;*** THIS IS NOT A SOURCE FILE ***
;*********************************

; This file was created by program 'buildprs' on Sat Jun 13 10:03:51 2026


PRSTAB_INC = ON ;Remember this file's included

ENCODE1BYTE     EQU     224
        ; all values from 0..223 can be output as 1 byte
        ; all values from 224..8415 are output as 2 bytes 
NTOKENS EQU     246     ; Total number of tokens
        ND_ACCEPT       EQU     00H
        ND_REJECT       EQU     01H
        ND_MARK         EQU     02H
        ND_EMIT         EQU     03H
        ND_BRANCH       EQU     04H

        STT_ACCEPT      EQU     0FFH

        NUMNTINT        EQU     29
        NUMNTEXT        EQU     49
        NUMNT           EQU     78

        RWF_OPERATOR    EQU     080H    ; reserved word is an operator
        RWF_FUNC        EQU     040H    ; reserved word can be a function
        RWF_STR         EQU     04H     ; reserved word ends with '$'
        RWF_FUNC_CG     EQU     010H    ; res word entry has func code generator
        RWF_STMT_CG     EQU     08H     ; res word entry has stmt code generator
        RWF_NO_DIRECT   EQU     020H    ; res word is illegal as direct mode stmt
        RWF_NSTMTS      EQU     03H     ; no. stmts beginning with this res word
        CB_RW_MAX       EQU     9       ; size of longest reserved word
        IRW_ALPHA_FIRST EQU     25      ; IRW for 1st non-special-char reserved word

STI_AsClausePrim             EQU 2504
STI_AsClause                 EQU 2552
STI_AsClauseAny              EQU 2560
sBegin  CP
        EXTRN   tRw:WORD        ;array mapping chars to ptrs into rw table
sEnd    CP
