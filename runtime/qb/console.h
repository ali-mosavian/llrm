/* The console as PRINT sees it (QB rt/iotty.asm B$TTY_*, rt/out.asm), and the
   screen statements that move and colour it.

   Where standard output is the screen, the console writes its text cells and
   keeps the cursor, the colours and the PRINT window itself; where it is a
   file or a pipe, it writes the bytes and only counts the column, and the
   screen statements do nothing. */
#ifndef QB_CONSOLE_H
#define QB_CONSOLE_H

#include "qb.h"

/* The cursor's 0-based column and the line width of the device PRINT writes to.
   */
byte cn_pos(void);
/* The cursor's 0-based row. */
byte cn_line(void);
byte cn_width(void);
/* Writes `n` bytes of `s`, a character at a time as B$OUTCNT does. */
void cn_write(const char *s, unsigned n);
void cn_putc(char c);
void cn_crlf(void);
/* Puts the screen's cursor where the console has it. */
void cn_sync(void);
/* The screen's cursor is shown while a program waits for typing and hidden
   while it runs, unless LOCATE asked for it; this is the wait. */
void cn_waiting(int waiting);
/* Erases the character before the cursor, which is on the same line. */
void cn_erase(void);

/* COLOR: a foreground 0-31 (16 and up blink) and a background 0-15 (8 and up
   are the colours 0 to 7), either -1 to leave it. */
void cn_color(int foreground, int background);
/* LOCATE: a 1-based row and column, either -1 to leave it, and whether the
   cursor is shown (-1 to leave it). */
void cn_locate(int row, int column, int cursor);
/* CLS: the PRINT window is cleared and the cursor goes to its top. */
void cn_cls(void);
/* VIEW PRINT: the 1-based rows of the PRINT window, or -1 and -1 for every
   row of the screen.  A program starts with all but the last. */
void cn_view(int top, int bottom);
/* WIDTH: only the screen's own size is accepted. */
void cn_set_size(int columns, int rows);

#endif
