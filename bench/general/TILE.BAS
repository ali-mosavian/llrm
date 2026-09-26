DEFINT A-Z
DECLARE FUNCTION Blit& (w, h, dx, dy)
'$DYNAMIC
DIM SHARED t(63, 63)
FOR y = 0 TO 63: FOR x = 0 TO 63: t(x, y) = (x * 3 + y * 5) AND 255: NEXT: NEXT
s& = Blit&(40, 25, 7, 61)
OPEN "TILE.OUT" FOR OUTPUT AS #1
PRINT #1, "RESULT="; s&
CLOSE #1
END

FUNCTION Blit& (w, h, dx, dy)
    DEF SEG = &HB800
    s& = 0
    o = 0
    FOR y = 0 TO h - 1
        FOR x = 0 TO w - 1
            v = t((x + dx) AND 63, (y + dy) AND 63)
            POKE o, v
            s& = s& + v
            o = o + 2
        NEXT
    NEXT
    DEF SEG
    Blit& = s&
END FUNCTION
