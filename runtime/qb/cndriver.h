/* The operations of one kind of console output (console.c, cntext.c,
   cngfx.c). */
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
    byte (*line)(void);
    byte (*width)(void);
    void (*color)(int foreground, int background);
    void (*locate)(int row, int column, int cursor);
    void (*clear)(void);
    void (*view)(int top, int bottom);
    void (*size)(int columns, int rows);
} Driver;

/* The drivers of the screen, set by the initializers of cntext.c and cngfx.c
   when a program has them linked, else 0 and everything goes to standard
   output. */
extern const Driver *cn_text_driver;
extern const Driver *cn_gfx_driver;

/* Whether the screen is in a graphics mode (set by gfx.c, which calls
   cn_mode_changed after). */
extern byte cn_graphics;

/* The driver in use, chosen when output first happens and again when the
   screen mode changes. */
const Driver *cn_driver(void);
void cn_mode_changed(void);

#endif
