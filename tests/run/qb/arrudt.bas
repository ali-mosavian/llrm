' Array of a TYPE both as a module-level DIM and as a SUB's own local --
' udt.bas only measured the module-level case; this is the BPREL half.
DEFINT A-Z
TYPE Coord
    x AS LONG
    y AS LONG
END TYPE

CONST N = 1
DIM pts(N) AS Coord

pts(0).x = 11
pts(0).y = 22
pts(1).x = 33
pts(1).y = 44

PRINT pts(0).x; pts(1).x
PRINT pts(0).y; pts(1).y

CALL Inside
PRINT "DONE"

SUB Inside
    DIM lpts(1) AS Coord
    lpts(0).x = 55
    lpts(0).y = 66
    lpts(1).x = 77
    lpts(1).y = 88
    PRINT lpts(0).x; lpts(1).x
    PRINT lpts(0).y; lpts(1).y
END SUB
