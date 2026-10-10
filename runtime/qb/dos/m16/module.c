/* The compiled module's header (QB B$GETMODCODE). */
#include "module.h"

extern unsigned qb_module_segment;

/* Offsets in the header (MODULE_CODE). */
enum ModuleWord {
    OF_MOD = 2,     /* the name, 8 characters */
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

void module_name(char *name)
{
    u8 QB_FAR *header = (u8 QB_FAR *)((unsigned long)qb_module_segment << 16);
    unsigned at;

    for (at = 0; at < MODULE_NAME_LENGTH; at++)
        name[at] = header[OF_MOD + at];
}

unsigned module_code_segment(void)
{
    return qb_module_segment;
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

unsigned md_on_error(const ModuleData *data)
{
    return data->on_error;
}

void md_set_on_error(ModuleData *data, unsigned offset)
{
    data->on_error = (u16)offset;
}
