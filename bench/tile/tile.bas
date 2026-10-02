' No POKE to video memory (C and Nib have no equivalent): the blit only sums.
DEFINT A-Z
DECLARE FUNCTION BenchTile& ()
DIM SHARED t(63, 63)
FOR y = 0 TO 63: FOR x = 0 TO 63: t(y, x) = (x * 3 + y * 5) AND 255: NEXT: NEXT
PRINT LTRIM$(STR$(BenchTile&))
END

FUNCTION BenchTile&
    w = 40: h = 25: dx = 7: dy = 61
    s& = 0
    FOR y = 0 TO h - 1
        FOR x = 0 TO w - 1
            s& = s& + t((y + dy) AND 63, (x + dx) AND 63)
        NEXT
    NEXT
    BenchTile& = s&
END FUNCTION
