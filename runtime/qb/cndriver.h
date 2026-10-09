/* The operations of one kind of console output (console.c, cntext.c). */
#ifndef QB_CNDRIVER_H
#define QB_CNDRIVER_H

#include "console.h"

typedef struct Driver {
    void (*init)(void);
    void (*write)(const char *text, unsigned count);
    void (*newline)(void);
    void (*erase)(void);
    void (*sync)(void);
    void (*cursor)(int visible);
    byte (*pos)(void);
    byte (*width)(void);
} Driver;

/* The text screen's driver, set by cntext.c's initializer when a program has
   it linked, else 0 and everything goes to standard output. */
extern const Driver *cn_text_driver;

/* The driver in use, chosen when output first happens. */
const Driver *cn_driver(void);

#endif
