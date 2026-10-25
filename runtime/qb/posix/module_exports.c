/* The compiled module through the symbols its object exports (llrm-qb --arm64): the code, the first DATA row and the
   statement table. The module's data area (the READ cursor, the ON ERROR handler) is the runtime's own, there being one
   module. */
#include "module.h"

extern void QB_MAIN(void);
extern const char QB_DATA[];
extern const char QB_STATEMENTS[];

/* The data area: a flag, the READ cursor, the ON ERROR handler. */
struct ModuleData {
    u8 restored;
    u8 unused;
    const char *data;
    unsigned on_error;
};

static ModuleData area;

ModuleData *module_data(void)
{
    return &area;
}

void module_name(char *name)
{
    unsigned at;

    for (at = 0; at < MODULE_NAME_LENGTH; at++)
        name[at] = " BASIC  "[at];
}

unsigned module_code_segment(void)
{
    return 0;
}

const char *module_first_data(void)
{
    /* the text of the first row, past its key (MODULE_CODE.OF_DS is BC_DS + 2) */
    return QB_DATA + 2;
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
    return data->data;
}

void md_set_cursor(ModuleData *data, const char *cursor)
{
    data->data = cursor;
}

unsigned md_on_error(const ModuleData *data)
{
    return data->on_error;
}

void md_set_on_error(ModuleData *data, unsigned offset)
{
    data->on_error = offset;
}

void qb_module_run(void)
{
    QB_MAIN();
}
