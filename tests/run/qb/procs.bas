' A SUB and a FUNCTION, so the module carries PUBDEF, a procedure prologue,
' and long operands that are bp-relative rather than static.
DEFINT A-Z
DECLARE FUNCTION Twice& (n AS LONG)
DECLARE SUB Report (tag AS STRING, n AS LONG)

DIM a AS LONG, b AS LONG, r AS LONG

a = 305419896
b = 252645135

r = a AND b
CALL Report("AND", r)
CALL Report("TWICE", Twice(r))
CALL Report("NESTED", Twice(Twice(r)))
PRINT "DONE"

FUNCTION Twice& (n AS LONG)
    DIM t AS LONG
    t = n + n
    Twice& = t
END FUNCTION

SUB Report (tag AS STRING, n AS LONG)
    PRINT tag; "="; n
END SUB
