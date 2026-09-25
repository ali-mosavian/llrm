' A chain of derived induction variables: i, i*12, i*12+5, and the
' address of a(q) -- four affine functions of one counter. Every one of
' them is a multiply BC does again on each pass, where an increment
' would do.
DEFINT A-Z
DIM a(300)
DIM i, p, q, t
t = 0
FOR i = 0 TO 20
    p = i * 12
    q = p + 5
    a(q) = i
    t = t + a(q)
NEXT i
PRINT "T="; t
PRINT "DONE"
