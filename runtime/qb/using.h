/* PRINT USING (QB rt/prnusing.asm, rt/prusing): the items of a PRINT laid out
   by a format string. */
#ifndef QB_USING_H
#define QB_USING_H

#include "nhstutil.h"

/* B$USNG starts a statement with a format; until it ends, the PRINT items go
   through these instead of being written as they are. */
int using_active(void);
void using_begin(SD *format);
void using_integer(long value);
void using_real(double value, int is_double);
void using_string(SD *item);
/* The statement is over: the text that follows the last field is written, and
   a line ends unless the statement ended with a separator. */
void using_end(int newline);

#endif
