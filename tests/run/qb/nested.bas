' Nested loops. i * w is invariant in the inner loop and BC recomputes it
' every pass; the element's own address is computed twice per statement
' because the subscript appears twice.
DEFINT A-Z
DIM b(60)
DIM i, j, w, h, t
w = 6
h = 5
t = 0
FOR i = 0 TO h - 1
    FOR j = 0 TO w - 1
        b(i * w + j) = i * 10 + j
        t = t + b(i * w + j)
    NEXT j
NEXT i
PRINT "T="; t
PRINT "DONE"
