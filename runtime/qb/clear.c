/* CLEAR (QB rt/clear.asm B$SCLR) on a flat target. Files are closed, the string space and the arrays in the local
   heap start afresh, sound stops and event traps go off; then every module variable is zeroed, from the table the
   compiler lists them in (the Microsoft runtime zeroes its data segment, BC_DATA, whole). Locals of procedures are not
   touched: a procedure's CLEAR is Illegal function call, its table saying so. The host's stack is fixed, so CLEAR's
   stack size is accepted and ignored. */
#include "file.h"
#include "nheap.h"
#include "qb.h"

#pragma aux B_SCLR "B$SCLR"

typedef struct ClearRow {
    char *address;
    uword bytes;
} ClearRow;

/* The module's flag, the rows' count, the rows. */
typedef struct ClearTable {
    uword main_module;
    uword count;
    ClearRow rows[1];
} ClearTable;

extern void music_clear(void);
extern void events_clear(void);

void B_SCLR(short stack, short given, const ClearTable *table)
{
    uword row, at;

    (void)stack;
    (void)given;
    if (!table->main_module)
        qb_error(BE_ILLFUN);
    file_close_all();
    music_clear();
    events_clear();
    nh_clear();
    for (row = 0; row < table->count; row++)
        for (at = 0; at < table->rows[row].bytes; at++)
            table->rows[row].address[at] = 0;
}
