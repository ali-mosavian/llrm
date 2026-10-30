/* The compiled module's header (QB B$GETMODCODE) on a host: machine-word fields where QB's has 16-bit offsets, 80 bytes
   with eight-byte words and the flat DOS target's 48 with four. The object defines the symbol QB_HEADER at its first
   byte; its code starts HEADER_SIZE bytes after. */
#include "module.h"

extern const u8 QB_HEADER[] __asm__("_QB_HEADER");

/* Offsets in the header. */
enum ModuleWord {
    OF_MOD = 2,                       /* the name, 8 characters */
    OF_DS = sizeof(uword) == 8 ? 24 : 16,  /* the READ/DATA lines */
    OF_DAT = sizeof(uword) == 8 ? 32 : 20, /* the module's data area */
    HEADER_SIZE = sizeof(uword) == 8 ? 80 : 48
};

/* The data area the header names: a flag, the READ cursor, the ON ERROR handler. */
struct ModuleData {
    u8 restored;
    u8 unused;
    const char *data;
    unsigned on_error;
};

static uword module_word(enum ModuleWord at)
{
    return *(const uword *)(QB_HEADER + at);
}

ModuleData *module_data(void)
{
    return (ModuleData *)module_word(OF_DAT);
}

void module_name(char *name)
{
    unsigned at;

    for (at = 0; at < MODULE_NAME_LENGTH; at++)
        name[at] = (char)QB_HEADER[OF_MOD + at];
}

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

/* The module's body: the runtime has started, the code follows the header. */
void qb_module_run(void)
{
    ((void (*)(void))(QB_HEADER + HEADER_SIZE))();
}
