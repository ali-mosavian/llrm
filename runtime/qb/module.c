/* The compiled module's header (QB B$GETMODCODE). */
#include "module.h"

extern word qb_module_segment;

/* B$GETMODCODE: a word of the module header. */
word module_word(enum ModuleWord at)
{
    word __far *header = (word __far *)((unsigned long)qb_module_segment << 16);

    return header[at / 2];
}

ModuleData *module_data(void)
{
    return (ModuleData *)module_word(OF_DAT);
}
