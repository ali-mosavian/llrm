/* VARPTR$ and the macro languages' use of it: DRAW and PLAY take a variable's
   value as `=` and the VARPTR$ string, and execute a string variable by X. */
#ifndef QB_VARPTR_H
#define QB_VARPTR_H

#include "qb.h"

/* After an `=` in a macro string: the VARPTR$ string at *at (a type byte and
   the address of the variable) names an integer or a number, which is returned
   as an integer; *at moves past it.  A string variable, or a string too short,
   is an Illegal function call. */
long varptr_integer(const char **at, const char *end);

/* After an `X`: the string variable the VARPTR$ string at *at names. */
const SD *varptr_string(const char **at, const char *end);

#endif
