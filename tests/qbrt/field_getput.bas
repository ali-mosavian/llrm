' GET and PUT move a RANDOM file's record buffer, FIELD's strings. The C runtime had no B$GET1/2 or B$PUT1/2, so such a program failed to link.
OPEN "FIELD.DAT" FOR RANDOM AS #1 LEN = 16
FIELD #1, 8 AS a$, 8 AS b$
LSET a$ = "alpha": LSET b$ = "beta"
PUT #1, 2
LSET a$ = "gamma": LSET b$ = "delta"
PUT #1, 1
GET #1, 2
PRINT a$; "|"; b$
GET #1, 1
PRINT a$; "|"; b$
GET #1
PRINT a$; "|"; b$
LSET a$ = "kept": LSET b$ = "buffer"
GET #1, 5
PRINT LEN(a$); LEN(b$); ASC(a$); ASC(MID$(a$, 5)); ASC(b$)
PRINT LOF(1)
CLOSE #1
KILL "FIELD.DAT"
PRINT LEN(a$)
END
