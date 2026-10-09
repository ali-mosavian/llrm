/* The console as PRINT sees it (QB rt/iotty.asm B$TTY_*, rt/out.asm). */
#ifndef QB_CONSOLE_H
#define QB_CONSOLE_H

#include "qb.h"

/* The cursor's 0-based column and the line width of the device PRINT writes to. */
byte cn_pos(void);
byte cn_width(void);
/* Write `n` bytes from `s`, a character at a time as B$OUTCNT does. */
void cn_write(const byte *s, word n);
void cn_putc(byte c);
void cn_crlf(void);

#endif
