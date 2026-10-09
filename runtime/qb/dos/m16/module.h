/* The compiled module (QB inc/addr.inc, rt/nhutil B$GETMODCODE): its header at
   offset 0 of its code segment, and the data area the header points at in
   DGROUP.  The portable code sees only the READ/DATA state. */
#ifndef QB_MODULE_H
#define QB_MODULE_H

#include "qb.h"

typedef struct ModuleData ModuleData;

/* The module's data area. */
ModuleData *module_data(void);

/* The first DATA line (MODULE_CODE.OF_DS). */
const char *module_first_data(void);

/* RESTORE has run, and the next DATA item. */
int md_restored(const ModuleData *data);
void md_set_restored(ModuleData *data);
const char *md_cursor(const ModuleData *data);
void md_set_cursor(ModuleData *data, const char *cursor);

#endif
