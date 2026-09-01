' A loop-invariant product, and a counter and accumulator that both
' round-trip through memory every iteration. BC computes n * k inside
' the loop twenty times, from two constants it already knows.
DEFINT A-Z
DIM n, k, s, i
n = 7
k = 3
s = 0
FOR i = 1 TO 20
    s = s + (n * k) + i
NEXT i
PRINT "S="; s
PRINT "DONE"
