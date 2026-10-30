' FIELD maps strings onto a RANDOM file's record buffer. The C runtime had no B$FLDP or B$FIEL, so such a program failed to
' link. LSET fills the buffer, a plain assignment takes a string out of it, CLOSE empties the fielded ones.
OPEN "FIELD.DAT" FOR RANDOM AS #1 LEN = 16
FIELD #1, 8 AS a$, 4 AS b$, 4 AS c$
LSET a$ = "alpha": LSET b$ = "beta": RSET c$ = "ab"
PRINT a$; "|"; b$; "|"; c$; "|"; LEN(a$); LEN(b$); LEN(c$)
c$ = "own string"
PRINT c$; "|"; LEN(c$)
LSET a$ = "x"
PRINT a$; "|"
CLOSE #1
KILL "FIELD.DAT"
PRINT LEN(a$); LEN(b$); "|"; c$
END
