/* A module with nothing in it, for a test that drives the runtime's C directly: no ON ERROR handler, no DATA. */
#include "module.h"

struct ModuleData {
    unsigned on_error;
};

static ModuleData data;

ModuleData *module_data(void)
{
    return &data;
}

void module_name(char *name)
{
    int at;

    for (at = 0; at < MODULE_NAME_LENGTH; at++)
        name[at] = "HOSTTEST"[at];
}

unsigned module_code_segment(void)
{
    return 0;
}

unsigned md_on_error(const ModuleData *module)
{
    return module->on_error;
}

void md_set_on_error(ModuleData *module, unsigned offset)
{
    module->on_error = offset;
}

const char *module_first_data(void)
{
    return 0;
}

int md_restored(const ModuleData *module)
{
    (void)module;
    return 0;
}

void md_set_restored(ModuleData *module)
{
    (void)module;
}

const char *md_cursor(const ModuleData *module)
{
    (void)module;
    return 0;
}

void md_set_cursor(ModuleData *module, const char *cursor)
{
    (void)module;
    (void)cursor;
}
