' Two-dimensional indexing by hand, which is what SCEV is for: r * w is
' invariant in the inner loop, r * w + c is affine in c, and the diagonal
' read in the second loop is affine in r with a stride of w + 1.
DEFINT A-Z
DIM m(400)
DIM r, c, w, t
w = 20
t = 0
FOR r = 0 TO 19
    FOR c = 0 TO 19
        m(r * w + c) = r + c
    NEXT c
NEXT r
FOR r = 0 TO 19
    t = t + m(r * w + r)
NEXT r
PRINT "T="; t
PRINT "DONE"
