' dialect: qb45
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
CLOSE #1
END
