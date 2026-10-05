' Element addresses at two strides, and a subscript that is not the
' counter. BC computes each one with a shift and a move into si; a 386
' addresses all of them in the instruction that uses them.
DEFINT A-Z
DIM a(100)
DIM b(100) AS LONG
DIM i, t
DIM u AS LONG
t = 0
u = 0
FOR i = 1 TO 20
    a(i) = i
    t = t + a(i) + a(i + 1)
    b(i) = i
    u = u + b(i)
NEXT i
PRINT "T="; t
PRINT "U="; u
PRINT "DONE"
