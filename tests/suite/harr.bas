' Two dimensions, dynamic. Each subscript pair goes through the array
' helper, which recomputes row * width * size + column * size as a long
' multiply on every use -- twice per statement here -- and reloads the
' segment each time. One add per pass is what an induction variable with
' its strength reduced would have cost.
REM $DYNAMIC
DEFINT A-Z
DIM m(20, 20)
DIM r, c, t
t = 0
FOR r = 1 TO 10
    FOR c = 1 TO 10
        m(r, c) = r + c
        t = t + m(r, c)
    NEXT c
NEXT r
PRINT "T="; t
PRINT "DONE"
