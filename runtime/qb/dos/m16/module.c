/* The compiled module's header (QB B$GETMODCODE). */
#include "module.h"

extern unsigned qb_module_segment;

/* Offsets in the header (MODULE_CODE). */
enum ModuleWord {
    OF_DS = 12,     /* the READ/DATA lines, in DGROUP */
    OF_DAT = 14     /* the module's data area, in DGROUP */
};

struct ModuleData {
    u8 restored;      /* RESTORE has run */
    u8 unused;
    u16 data;         /* the next DATA item */
    u16 on_error;     /* the ON ERROR handler, 0 for none */
};

/* B$GETMODCODE: a word of the module header. */
static u16 module_word(enum ModuleWord at)
{
    u16 QB_FAR *header = (u16 QB_FAR *)((unsigned long)qb_module_segment << 16);

    return header[at / 2];
}

ModuleData *module_data(void)
{
    return (ModuleData *)module_word(OF_DAT);
}

const char *module_first_data(void)
{
    return (const char *)module_word(OF_DS);
}

int md_restored(const ModuleData *data)
{
    return data->restored;
}

void md_set_restored(ModuleData *data)
{
    data->restored = 1;
}

const char *md_cursor(const ModuleData *data)
{
    return (const char *)data->data;
}

void md_set_cursor(ModuleData *data, const char *cursor)
{
    data->data = (u16)cursor;
}
