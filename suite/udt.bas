' TYPE with two LONG fields, a plain DIM of it, and an array of it -- the
' structure and array-of-structure shapes in $$TYPES that qbopt/cvinfo.py
' decodes, named "udt" (kept to 8.3-safe length for suite/e2e.py's B_/O_ prefix).
DEFINT A-Z
TYPE Coord
    x AS LONG
    y AS LONG
END TYPE

CONST N = 2
DIM c AS Coord
DIM pts(N) AS Coord

c.x = 305419896
c.y = 252645135
pts(0).x = 1
pts(0).y = 2
pts(1).x = 3
pts(1).y = 4
pts(2).x = 5
pts(2).y = 6

PRINT c.x; c.y
PRINT pts(0).x; pts(0).y; pts(1).x; pts(1).y; pts(2).x; pts(2).y
PRINT "DONE"
END
