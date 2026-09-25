' fpcse, with the operands read at runtime. (a + b) is still a shared
' subexpression and p and q still round-trip through memory.
DEFSNG A-Z
DIM a, b, c, p, q, s
DIM i AS INTEGER
DATA 2, 4, 8
READ a, b, c
s = 0
FOR i = 1 TO 10
    p = (a + b) * c
    q = (a + b) / c
    s = s + p + q
NEXT i
PRINT "S="; s
PRINT "DONE"
