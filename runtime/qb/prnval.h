/* What the PRINT item routines share (prnval.c, prnreal.c), and the hook by
   which PRINT USING takes the items once its format is set: the code of USING
   is linked only by a program that uses it. */
#ifndef QB_PRNVAL_H
#define QB_PRNVAL_H

#include "nhstutil.h"

enum Terminator { COMMA, SEMI, EOL };

/* A number's text, its trailing space, and the terminator. */
void print_numeral(char *text, unsigned length, enum Terminator end);

typedef struct UsingOps {
    void (*integer)(long value);
    void (*real)(double value, int is_double);
    void (*string)(SD *item);
    void (*end)(int newline);
} UsingOps;

/* Set between B$USNG and the end of its statement, else 0. */
extern const UsingOps *using_ops;

/* The end of an item of a PRINT USING: a separator adds nothing, and the end of
   the statement finishes the format. */
void using_item_end(enum Terminator end);

#endif
