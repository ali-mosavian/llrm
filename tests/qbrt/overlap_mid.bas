' MID$ of a string into itself, the target a byte or two above the source: a forward copy a byte at a time
' smears the first characters (QB's own result); a copy by words did not.
a$ = "abcdefghij"
MID$(a$, 2) = a$
PRINT a$
b$ = "abcdefghij"
MID$(b$, 4) = b$
PRINT b$
c$ = "abcdefghij"
MID$(c$, 6) = c$
PRINT c$
