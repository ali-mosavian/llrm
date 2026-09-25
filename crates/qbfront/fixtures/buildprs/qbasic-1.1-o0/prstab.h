/* Quick BASIC Interpreter - module 'prstab.h' - (c) Microsoft Corp. 1985

 NOTE *********************************
      *** THIS IS NOT A SOURCE FILE ***
      *********************************

   This file was created by program 'buildprs' on Sat Jun 13 10:03:51 2026


*/
#undef PRSTAB_H
#define PRSTAB_H ON	/* Remember this file's included */

#define ENCODE1BYTE   224
		/* all values from 0..223 can be output as 1 byte */
		/* all values from 224..8415 are output as 2 bytes */
#define NTOKENS 246 /* Total number of tokens */

#define ND_ACCEPT 0x0
#define ND_REJECT 0x1
#define ND_MARK  0x2
#define ND_EMIT  0x3
#define ND_BRANCH  0x4

#define STT_ACCEPT 0xFF

#define NUMNTINT 29
#define NUMNTEXT 49
#define NUMNT   78

#define RWF_OPERATOR 0x80 /* true if reserved word is an operator */
#define RWF_FUNC 0x40 /* true if reserved word can be a function */
#define RWF_STR 0x4 /* true if reserved word ends with '$' */
#define RWF_FUNC_CG 0x10 /* true if res word entry has func code generator */
#define RWF_STMT_CG 0x8 /* true if res word entry has stmt code generator */
#define RWF_NO_DIRECT 0x20 /* res word is illegal as direct mode stmt */
#define RWF_NSTMTS 0x3 /* no. stmts beginning with this res word */
#define CB_RW_MAX 9 /* size of longest reserved word */
#define IRW_ALPHA_FIRST 25 /* IRW for 1st non-special-char reserved word */

#define STI_AsClausePrim             2504

#define STI_AsClause                 2552

#define STI_AsClauseAny              2560
