/* The QB runtime's shared types.  An entry is a plain public function, which the medium model makes far
   and llrm-qb calls (-fqb-runtime=llrm) with the target's own convention; its link name is the B$ name
   a `#pragma aux` gives it, since `$` is not a C identifier. */
#ifndef QB_H
#define QB_H

#define NULL ((void *)0)

typedef unsigned char byte;
typedef unsigned int word;

/* A string descriptor (inc/string.inc): the length, and the data's address in DGROUP. */
typedef struct SD {
    word len;
    char *ptr;
} SD;

/* Value types of the frontend's numeric ABI (inc/rtps.inc VT_*). */
enum { VT_I2 = 0x02, VT_SD = 0x03, VT_R4 = 0x04, VT_R8 = 0x08, VT_I4 = 0x14 };

/* Run-time errors (inc/messages.inc BE_*) and the fatal ones (FE_*). */
enum {
    BE_RETURN = 3, BE_NODATA = 4, BE_ILLFUN = 5, BE_OVERFLOW = 6, BE_MEMORY = 7, BE_SUBSCRIP = 9,
    BE_REDIM = 10, BE_DIVIDE0 = 11, BE_TYPE = 13, BE_STRINGSP = 14, BE_STRINGFO = 16, BE_NORESUME = 19,
    BE_RESUME = 20, BE_FILENUM = 52, BE_NOFILE = 53, BE_FILEMODE = 54, BE_FILEOPEN = 55, BE_DEVICEIO = 57,
    BE_EXISTS = 58, BE_DISKFULL = 61, BE_PASTEND = 62, BE_BADREC = 63, BE_BADNAME = 64, BE_TOOMANY = 67,
    BE_HANDSOFF = 70, BE_NOTFOUND = 76,
    FE_CORRUPT = 0x9000, FE_NOSTACK = 0x9007
};

static void copy_bytes(
    char *to,
    const char *from,
    word n)
{
    while (n--)
        *to++ = *from++;
}

/* error.c: raise run-time error `n`.  It does not return: control goes to the ON ERROR handler, or the
   program ends. */
void qb_error(word n);

#endif
