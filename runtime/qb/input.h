/* INPUT from the console (QB rt/inptty.asm B$INPP): the line typed, which
   READ's item routines then take their items from (read.c). */
#ifndef QB_INPUT_H
#define QB_INPUT_H

#include "qb.h"

/* Whether a line of INPUT is waiting to be taken apart. */
int input_active(void);
/* Where the next item starts in it. */
const char *input_cursor(void);
void input_set_cursor(const char *cursor);
/* B$PEOS ends the statement. */
void input_end(void);

#endif
