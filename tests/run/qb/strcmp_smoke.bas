' flags: -O1 | -O2 | -O3 | -Os
' String relations through the runtime's compare, in the answer smoke at every level: the empty string against a non-empty one,
' a prefix, and equal strings.
a$ = "abc": b$ = "abd": c$ = "ab": e$ = ""
PRINT "" < "a"; e$ < a$; a$ < b$; b$ < a$; c$ < a$; a$ < c$
PRINT e$ = e$; a$ = a$; a$ <> c$; a$ > e$; a$ + b$ < b$ + a$
