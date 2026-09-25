' press, with the eight variables read at runtime. Every product is still
' loop-invariant; none of them is a constant.
DEFINT A-Z
DIM a, b, c, d, e, f, g, h, r, i
DATA 3, 5, 7, 11, 13, 17, 19, 23
READ a, b, c, d, e, f, g, h
r = 0
FOR i = 1 TO 10
    r = r + a * b + c * d + e * f + g * h
NEXT i
PRINT "R="; r
PRINT "DONE"
