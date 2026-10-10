/* The compiled module's header (QB B$GETMODCODE), as the frontend lays it out for a flat target: dwords
   where QB's has 16-bit offsets, the same 48 bytes. */
#include "module.h"

/* start.asm: the module's header. */
extern const u8 *qb_module_header;
#pragma aux qb_module_header "qb_module_header"

/* Offsets in the header. */
enum ModuleWord {
    OF_MOD = 2,     /* the name, 8 characters */
    OF_DS = 16,     /* the READ/DATA lines */
    OF_DAT = 20     /* the module's data area */
};

/* The data area the header names: a flag, the READ cursor, the ON ERROR handler. */
struct ModuleData {
    u8 restored;
    u8 unused;
    const char *data;
    unsigned on_error;
};

static u32 module_word(enum ModuleWord at)
{
    return *(const u32 *)(qb_module_header + at);
}

ModuleData *module_data(void)
{
    return (ModuleData *)module_word(OF_DAT);
}

void module_name(char *name)
{
    unsigned at;

    for (at = 0; at < MODULE_NAME_LENGTH; at++)
        name[at] = qb_module_header[OF_MOD + at];
}

/* The part of a fatal error's address that stands for the segment: the code is flat, so the
   address is all offset. */
unsigned module_code_segment(void)
{
    return 0;
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
