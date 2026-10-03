DECLARE FUNCTION Min% (BYVAL x AS INTEGER, BYVAL y AS INTEGER)
DECLARE FUNCTION Dot& (a() AS INTEGER, b() AS INTEGER)

n = 9: m = 7
REDIM p(n) AS INTEGER, q(m) AS INTEGER
PRINT Dot&(p(), q())

FUNCTION Min% (BYVAL x AS INTEGER, BYVAL y AS INTEGER)
    IF x < y THEN Min% = x ELSE Min% = y
END FUNCTION

FUNCTION Dot& (a() AS INTEGER, b() AS INTEGER)
    DIM total AS LONG, i AS INTEGER
    FOR i = 0 TO Min%(UBOUND(a), UBOUND(b))
        total = total + CLNG(a(i)) * b(i)
    NEXT
    Dot& = total
END FUNCTION
