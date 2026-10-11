' A string is as long as memory on a flat target: its length is a word of the target, not 16 bits. Three
' 30000-byte strings joined are 90000 bytes; the last character and a middle one are still there.
a$ = SPACE$(29999) + "z"
b$ = a$ + a$ + a$
PRINT RIGHT$(b$, 1); ASC(MID$(b$, 30000, 1)); ASC(LEFT$(RIGHT$(b$, 2), 1))
PRINT LEN(b$); INSTR(60001, b$, "z"); LEN(LEFT$(b$, 70000))
