/* Disk files (QB rt/dkopen.asm, dkio.asm, dkutil.asm, dvstmt.asm).

   Each open file is a file data block (inc/fdb.inc) in a local heap entry that
   records its channel, so the files are found by walking the heap. */
#ifndef QB_FILE_H
#define QB_FILE_H

#include "nheap.h"

/* The modes of OPEN, as the frontend passes them (inc/devdef.inc MD_*). */
enum FileMode { MD_INPUT = 1, MD_OUTPUT = 2, MD_RANDOM = 4, MD_APPEND = 8, MD_BINARY = 32 };

/* Fdb.flags (inc/files.inc). */
enum { FL_FIELD = 0x08 };

typedef struct Fdb {
    SD fielded;   /* the FIELD back-pointer string: the address of each fielded SD, a word each */
    short handle; /* the OS's */
    byte mode;    /* an enum FileMode */
    byte flags;
    /* where the next byte of PRINT # goes on its line */
    byte column;
    byte past; /* a BINARY file's last read came up short: what EOF means there */
    unsigned record_length; /* RANDOM's record, INPUT's and OUTPUT's buffer */
} Fdb;

/* A RANDOM file's record buffer, which FIELD maps its strings onto: it follows the Fdb. */
#define FDB_BUFFER(fdb) ((char *)((Fdb *)(fdb) + 1))

/* B$OPEN, B$CLOS, B$GETn, B$PUTn, B$FLOF, B$FLOC, B$FEOF */
void B_OPEN(SD *name, short channel, short record_length, unsigned short mode);
long B_FATR(short channel, short field);
void B_KILL(SD *name);
void B_NAME(SD *old, SD *new_name);
void B_MDIR(SD *name);
void B_CDIR(SD *name);
void B_RDIR(SD *name);
void file_close(const int *channels, unsigned count);
void B_GET3(short channel, qb_data_ptr record, short length);
void B_PUT3(short channel, qb_data_ptr record, short length);
long B_FLOF(short channel);
long B_FLOC(short channel);
long B_FSEK(short channel);
void B_CHOU(short channel);
void file_print_to(Fdb *fdb);
void B_DSKI(short channel);
int B_FEOF(short channel);
int B_FREF(void);
void B_GET4(short channel, long record, qb_data_ptr data, short length);
void B_PUT4(short channel, long record, qb_data_ptr data, short length);
void B_SSEK(short channel, long position);
void B_FLDP(short channel);
void B_GET1(short channel);
void B_PUT1(short channel);
void B_GET2(short channel, long record);
void B_PUT2(short channel, long record);
void B_FIEL(short width, SD *variable);

#endif
