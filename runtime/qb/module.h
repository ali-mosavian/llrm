/* The compiled module (QB inc/addr.inc, rt/nhutil B$GETMODCODE): its header
   at offset 0 of its code segment, and the data area the header points at
   in DGROUP. */
#ifndef QB_MODULE_H
#define QB_MODULE_H

#include "qb.h"

/* Offsets in the header (MODULE_CODE). */
enum ModuleWord {
    OF_DS = 12,     /* the READ/DATA lines, in DGROUP */
    OF_DAT = 14     /* the module's data area, in DGROUP */
};

typedef struct ModuleData {
    byte restored;      /* RESTORE has run */
    byte unused;
    word data;          /* the next DATA item */
    word on_error;      /* the ON ERROR handler, 0 for none */
} ModuleData;

word module_word(enum ModuleWord at);
ModuleData *module_data(void);

#endif
