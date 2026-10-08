' dialect: pds71
' flags: -O2 -march=i486 -fsanitize=bounds,signed-integer-overflow --huge-arrays
' /D: each subscript is checked against its dimension and an unallocated
' array is refused, ERROR 9, as B$HARY checked them, here in code. A LONG
' subscript is narrowed to INTEGER first, ERROR 6. The .out is BC /D's.
DEFINT A-Z
DIM s(1 TO 10)
n = 5
REDIM d(-2 TO n, 3)
ON ERROR GOTO bad
s(10) = 1: d(-2, 0) = 4: d(5, 3) = 6
PRINT s(10); d(-2, 0); d(5, 3)
PRINT s(11)
PRINT s(0)
PRINT d(6, 0)
PRINT d(-3, 0)
PRINT d(0, 4)
i& = 70000
PRINT d(i&, 0)
ERASE d
PRINT d(0, 0)
PRINT "DONE"
END
bad:
PRINT "ERR"; ERR
RESUME NEXT
