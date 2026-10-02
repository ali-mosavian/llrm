' A SUB taking an array parameter, plain and of a TYPE -- arrays are always
' passed by descriptor; does that share ARRAY's own $$TYPES shape or wrap it?
DEFINT A-Z
TYPE Coord
    x AS LONG
    y AS LONG
END TYPE

DECLARE SUB FillNums (arr() AS LONG)
DECLARE SUB FillPts (arr() AS Coord)

DIM nums(1) AS LONG
DIM pts(1) AS Coord

CALL FillNums(nums())
CALL FillPts(pts())

PRINT nums(0); nums(1)
PRINT pts(0).x; pts(0).y; pts(1).x; pts(1).y
PRINT "DONE"

SUB FillNums (arr() AS LONG)
    arr(0) = 7
    arr(1) = 8
END SUB

SUB FillPts (arr() AS Coord)
    arr(0).x = 1
    arr(0).y = 2
    arr(1).x = 3
    arr(1).y = 4
END SUB
