' LINE -(x, y) and LINE -STEP(dx, dy): from the last point
DEFINT A-Z
DECLARE SUB Mark ()
DIM SHARED hue
DIM arr(200)
OPEN "L.TXT" FOR OUTPUT AS #1
SCREEN 13
Mark
PSET (10, 20), 1: Mark
LINE -(100, 100), 3: Mark
LINE -STEP(5, 7), 3: Mark
LINE (100, 120)-(110, 130), , B: Mark
LINE -(120, 140), 5: Mark
FOR y = 0 TO 199
    sum& = 0
    FOR x = 0 TO 319: sum& = (sum& * 31 + POINT(x, y) + 1) AND 16777215: NEXT
    PRINT #1, y; sum&
NEXT
CLOSE #1
END
SUB Mark
    PSET STEP(0, 0), hue
    hue = hue MOD 15 + 1
END SUB
