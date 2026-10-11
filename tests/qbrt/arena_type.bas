' A shared array of a TYPE of three integers: elements keep their fields apart (NIBBLES's arena).
TYPE arenaType
    realRow AS INTEGER
    acolor AS INTEGER
    sister AS INTEGER
END TYPE
DECLARE SUB InitColors ()
DECLARE FUNCTION PointIsThere (row, col, acolor)
DIM SHARED arena(1 TO 50, 1 TO 80) AS arenaType
DIM SHARED colorTable(10)
FOR a = 1 TO 6: READ colorTable(a): NEXT a
DATA 14, 13, 12, 1, 7, 4
InitColors
arena(3, 4).acolor = 7
arena(50, 80).realRow = 25
arena(1, 1).sister = -1
PRINT PointIsThere(3, 4, 1); PointIsThere(3, 5, 1); PointIsThere(50, 80, 1); PointIsThere(0, 0, 1)
PRINT arena(3, 4).acolor; arena(3, 5).acolor; arena(50, 80).realRow; arena(50, 80).acolor; arena(1, 1).sister; arena(1, 2).sister

SUB InitColors
    FOR row = 1 TO 50
        FOR col = 1 TO 80
            arena(row, col).acolor = colorTable(4)
        NEXT col
    NEXT row
END SUB

FUNCTION PointIsThere (row, col, acolor)
    IF row <> 0 THEN
        IF arena(row, col).acolor <> acolor THEN
            PointIsThere = -1
        ELSE
            PointIsThere = 0
        END IF
    END IF
END FUNCTION
