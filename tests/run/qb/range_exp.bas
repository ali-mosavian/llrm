' flags: -O1 | -O2 | -O3 | -Os
' dialect: qb45
' EXP raises Overflow (error 6) once x * log2(e) rounds past 32767: 22712.6 is INF, 22712.8 is error 6; negative arguments give 0 (real BC 4.5).
ON ERROR GOTO h
DIM x AS DOUBLE, s AS SINGLE, z AS DOUBLE
RESTORE d1
FOR k% = 1 TO 8
    READ x
    z = -1
    PRINT x;
    z = EXP(x)
    PRINT "ok"
    PRINT "."
NEXT
RESTORE d2
FOR k% = 1 TO 4
    READ s
    PRINT s;
    z = EXP(s)
    PRINT "ok"
    PRINT "."
NEXT
END
h:
PRINT "error"; ERR
RESUME NEXT
d1: DATA 22712.6,22712.8,23000,100000,-23000,-100000,0,1D+300
d2: DATA 22712,22713,-22713,100000
