/* Disk files (QB rt/dkopen.asm, dkio.asm, dkutil.asm, dvstmt.asm).

   Each open file is a file data block (inc/fdb.inc) in a local heap entry that
   records its channel, so the files are found by walking the heap. */
#ifndef QB_FILE_H
#define QB_FILE_H

#include "nheap.h"

/* The modes of OPEN, as the frontend passes them (inc/devdef.inc MD_*). */
enum FileMode {
    MD_INPUT = 1,
    MD_OUTPUT = 2,
    MD_RANDOM = 4,
    MD_APPEND = 8,
    MD_BINARY = 32
};

typedef struct Fdb {
    SD fielded;           /* the FIELD back-pointer string, if any */
    short handle;         /* the OS's */
    byte mode;            /* an enum FileMode */
    byte flags;
    byte column;          /* where the next byte of PRINT # goes on its line */
    unsigned record_length;   /* RANDOM's record, INPUT's and OUTPUT's buffer */
} Fdb;

/* B$OPEN, B$CLOS, B$GETn, B$PUTn, B$FLOF, B$FLOC, B$FEOF */
void B_OPEN(SD *name, int channel, int record_length, unsigned mode);
void file_close(const int *channels, unsigned count);
void B_GET3(int channel, qb_data_ptr record, int length);
void B_PUT3(int channel, qb_data_ptr record, int length);
long B_FLOF(int channel);
void B_CHOU(int channel);
void file_print_to(Fdb *fdb);
void B_DSKI(int channel);
int B_FEOF(int channel);
int B_FREF(void);
void B_GET4(int channel, long record, int length, qb_data_ptr data);
void B_PUT4(int channel, long record, int length, qb_data_ptr data);
void B_SSEK(int channel, long position);

#endif
