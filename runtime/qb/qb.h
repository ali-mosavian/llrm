/* The QB runtime's shared types.  An entry is a plain public function, which
   the medium model makes far and llrm-qb calls (-fqb-runtime=llrm) with the
   target's own convention; its link name is the B$ name a `#pragma aux` gives
   it, since `$` is not a C identifier. */
#ifndef QB_H
#define QB_H

#include "platform.h"

#define NULL ((void *)0)

typedef u8 byte;

/* A string descriptor (inc/string.inc): the length, and the data's address in
   DGROUP. */
/* The most bytes a string holds: its length is a word, and the entry round it
 * must not wrap. */
enum { SD_MAX_LENGTH = (uword)~0u - 8 };

typedef struct SD {
    uword len;
    char *ptr;
} SD;

/* Value types of the frontend's numeric ABI (inc/rtps.inc VT_*). */
enum { VT_I2 = 0x02, VT_SD = 0x03, VT_R4 = 0x04, VT_R8 = 0x08, VT_I4 = 0x14 };

/* Run-time errors (inc/messages.inc BE_*) and the fatal ones (FE_*). */
#define FE_CORRUPT 0x9000L
#define FE_NOSTACK 0x9007L
enum {
    BE_SYNTAX = 2,
    BE_RETURN = 3,
    BE_NODATA = 4,
    BE_ILLFUN = 5,
    BE_OVERFLOW = 6,
    BE_MEMORY = 7,
    BE_SUBSCRIP = 9,
    BE_REDIM = 10,
    BE_DIVIDE0 = 11,
    BE_TYPE = 13,
    BE_STRINGSP = 14,
    BE_STRINGFO = 16,
    BE_NORESUME = 19,
    BE_RESUME = 20,
    BE_FIELD = 50,
    BE_FILENUM = 52,
    BE_NOFILE = 53,
    BE_FILEMODE = 54,
    BE_FILEOPEN = 55,
    BE_DEVICEIO = 57,
    BE_EXISTS = 58,
    BE_DISKFULL = 61,
    BE_PASTEND = 62,
    BE_BADREC = 63,
    BE_BADNAME = 64,
    BE_TOOMANY = 67,
    BE_UNAVAILABLE = 68,
    BE_HANDSOFF = 70,
    BE_RENAMEDISK = 74,
    BE_ACCESS = 75,
    BE_NOTFOUND = 76
};

#pragma aux copy_bytes "B$COPY"
#pragma aux fill_bytes "B$FILLB"
#pragma aux compare_bytes "B$CMPB"
#pragma aux qb_error "B$ERROR"

/* `n` bytes copied forward, a word at a time (the target's block.asm); where
   the regions overlap, `to` is below `from`. */
void copy_bytes(char *to, const char *from, unsigned n);

/* `n` bytes at `to` set to `value`, by dwords (block.asm). */
void fill_bytes(char *to, int value, unsigned n);

/* -1, 0 or 1 as the `n` bytes at `a` sort before, with or after those at `b`,
   as unsigned (block.asm). */
int compare_bytes(const char *a, const char *b, unsigned n);

/* error.c: raise run-time error `n`.  It does not return: control goes to the
   ON ERROR handler, or the program ends. */
void qb_error(unsigned n);

#endif
