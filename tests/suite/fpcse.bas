' A subexpression shared between two statements, and both results used in
' a third. BC compiles a statement at a time, so (a + b) is computed
' twice and p and q each go to memory and come straight back -- with an
' x87 stack eight deep that it never uses more than two of.
DEFSNG A-Z
DIM a, b, c, p, q, s
DIM i AS INTEGER
a = 2
b = 4
c = 8
s = 0
FOR i = 1 TO 10
    p = (a + b) * c
    q = (a + b) / c
    s = s + p + q
NEXT i
PRINT "S="; s
PRINT "DONE"
