/* VARPTR$ and the macro languages' use of it: DRAW and PLAY take a variable's
   value as `=` and the VARPTR$ string, and execute a string variable by X. */
#ifndef QB_VARPTR_H
#define QB_VARPTR_H

#include "qb.h"

/* After an `=` in a macro string: the VARPTR$ string at *at (a type byte and
   the address of the variable) names an integer or a number, which is returned
   as an integer; *at moves past it.  A string, a LONG (BC refuses it), or a string too short,
   is an Illegal function call. */
long varptr_integer(const char **at, const char *end);

/* After an `X`: the string variable the VARPTR$ string at *at names. */
const SD *varptr_string(const char **at, const char *end);

/* The scanner DRAW and PLAY share (QB rt/mclprc.asm B$MACLNG, B$MCLXEQ for X, B$VALSCN for `=`): a string of
   commands, and how deep the X commands run strings within it. */
typedef struct Macro {
    const char *at, *end;
    int depth;
} Macro;

/* The string `commands` is about to be executed from its start. */
void macro_begin(Macro *m, const SD *commands);

/* The next command letter in upper case, blanks and semicolons passed; 0 at the end. */
char macro_next(Macro *m);

/* A `=` at the scan point, then a VARPTR$ string naming an integer or a number: true, the variable's value rounded to an
   integer in *value, the scan point past it.  False where there is no `=`. */
int macro_equals(Macro *m, long *value);

/* After an `X`: the string variable the VARPTR$ string names becomes the string being scanned, `outer` keeps where the
   scan was; more than 16 deep is an Illegal function call.  macro_leave puts the scan back. */
void macro_enter(Macro *m, Macro *outer);
void macro_leave(Macro *m, const Macro *outer);

#endif
