' DATE$ and TIME$, read and set: the forms the statements accept and the
' errors they raise.
DEFINT A-Z
OPEN "L.TXT" FOR OUTPUT AS #1
DATE$ = "02-03-2001"
PRINT #1, DATE$
DATE$ = "7/4/99"
PRINT #1, DATE$
DATE$ = "12-31-1999"
PRINT #1, DATE$; LEN(DATE$)
TIME$ = "10:20:15"
a$ = TIME$
PRINT #1, LEFT$(a$, 5); LEN(a$)
TIME$ = "5:30:30"
PRINT #1, LEFT$(TIME$, 5)
TIME$ = "6.07.30"
PRINT #1, LEFT$(TIME$, 5)
ON ERROR GOTO oops
DATE$ = "13-01-2000"
DATE$ = "1-1"
DATE$ = "01-01-1979"
TIME$ = "25:00:00"
TIME$ = "10:99:00"
TIME$ = "x:00:00"
CLOSE #1
END

oops:
  PRINT #1, "error"; ERR
  RESUME NEXT
