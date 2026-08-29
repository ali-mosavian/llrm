' BYREF SINGLE and DOUBLE parameters -- procs.bas only measured LONG/INTEGER/
' STRING; this extends QB45_BYREF_PRIMITIVES if the pattern holds.
DEFINT A-Z
DECLARE FUNCTION Half! (n AS SINGLE)
DECLARE FUNCTION Doubled# (n AS DOUBLE)

DIM s AS SINGLE, d AS DOUBLE
s = 4.0
d = 8.0

PRINT Half(s)
PRINT Doubled(d)
PRINT "DONE"

FUNCTION Half! (n AS SINGLE)
    Half! = n / 2
END FUNCTION

FUNCTION Doubled# (n AS DOUBLE)
    Doubled# = n * 2
END FUNCTION
