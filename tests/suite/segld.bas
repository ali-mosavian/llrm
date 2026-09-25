' A dynamic array in a nested loop. Every element access goes through the
' descriptor and reloads es, on every pass of the inner loop, from a
' descriptor nothing in either loop writes.
REM $DYNAMIC
DEFINT A-Z
DIM a(100)
DIM i, j, t
t = 0
FOR j = 1 TO 5
    FOR i = 1 TO 20
        a(i) = i
        t = t + a(i)
    NEXT i
NEXT j
PRINT "T="; t
PRINT "DONE"
