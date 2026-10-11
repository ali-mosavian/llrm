' PSET PRESET LINE CIRCLE GET PUT with STEP, whole and fractional: the last point after each
DEFINT A-Z
DECLARE SUB Mark ()
DIM SHARED hue
DIM arr(200)
OPEN "L.TXT" FOR OUTPUT AS #1
SCREEN 13
Mark
PSET (10, 20), 1: Mark
PSET STEP(5, 5), 2: Mark
PRESET (30, 40): Mark
PRESET STEP(-3, 2): Mark
LINE (50, 60)-(70, 80), 3: Mark
LINE (50, 60)-STEP(10, 12), 3: Mark
LINE (10, 10)-(20, 25), 3, B: Mark
LINE (10, 10)-(20, 25), 3, BF: Mark
LINE STEP(5, 5)-STEP(5, 5), 3: Mark
LINE (80, 30)-(60, 10), 3: Mark
CIRCLE (90, 90), 5, 3: Mark
CIRCLE STEP(2, 3), 4, 3: Mark
PAINT (95, 95), 2, 3: Mark
PSET (150, 150), 0
GET (10, 10)-(20, 20), arr: Mark
GET STEP(5, 5)-STEP(10, 10), arr: Mark
PUT (30, 30), arr, PSET: Mark
PUT STEP(4, 4), arr, PSET: Mark
PSET (10, 10), 1
PSET STEP(.5, .5), 1: Mark
PSET STEP(.5, .5), 1: Mark
PSET STEP(.4, .4), 1: Mark
PSET STEP(.4, .4), 1: Mark
PSET STEP(.4, .4), 1: Mark
PSET (10.5, 11.5), 1: Mark
PSET (-5, 300), 1: Mark
PSET STEP(2, 2), 1: Mark
LINE (-10, -10)-(400, 300), 3: Mark
CIRCLE (319, 199), 4, 3: Mark
PSET STEP(1, 1), 1: Mark
PSET (11, 11), 1: PSET STEP(2.5, 2.5), 1: Mark
PSET STEP(3.5, 3.5), 1: Mark
PSET STEP(-1.5, -2.5), 1: Mark
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
