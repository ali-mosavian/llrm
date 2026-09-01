' Eight variables live at once, and every product in the loop body is
' loop-invariant. BC keeps nothing in a register across a statement, so
' each of the sixteen operands is loaded from memory on every pass.
DEFINT A-Z
DIM a, b, c, d, e, f, g, h, r, i
a = 3
b = 5
c = 7
d = 11
e = 13
f = 17
g = 19
h = 23
r = 0
FOR i = 1 TO 10
    r = r + a * b + c * d + e * f + g * h
NEXT i
PRINT "R="; r
PRINT "DONE"
