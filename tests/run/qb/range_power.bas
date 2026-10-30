' flags: -O1 | -O2 | -O3 | -Os
' dialect: qb45
' x ^ y raises Overflow (error 6) where y * log2(x) rounds past 32767, Illegal function call (error 5) for 0^-1 and a negative base with a fractional exponent; INF below that (real BC 4.5).
ON ERROR GOTO h
DIM x AS DOUBLE, y AS DOUBLE, z AS DOUBLE
RESTORE d1
FOR k% = 1 TO 10
    READ x, y
    PRINT x; y;
    z = x ^ y
    PRINT "ok"
    PRINT "."
NEXT
END
h:
PRINT "error"; ERR
RESUME NEXT
d1: DATA 2,100000, 10,22712, 10,500, 1D+200,2, 10,-400, 0,-1, -8,.3333333333333333, -2,3, 2,32767, 2,32768
