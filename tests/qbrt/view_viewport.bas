' VIEW: points count from the viewport's corner (not VIEW SCREEN), drawing is cut to it, the border is drawn outside it.
DEFINT A-Z
DECLARE SUB dump ()
OPEN "L.TXT" FOR OUTPUT AS #1
SCREEN 7
VIEW (40, 30)-(139, 99), 2, 3
PSET (0, 0), 5
LINE (-20, -20)-(300, 200), 4
LINE (10, 60)-(90, 10), 1, B
CIRCLE (50, 40), 80, 6
dump
PAINT (5, 55), 9, 6
PSET STEP(0, 0), 1
dump
VIEW
PRINT #1, POINT(39, 29); POINT(40, 30); POINT(140, 100); POINT(139, 99); POINT(0, 0)
VIEW SCREEN (40, 30)-(139, 99), , 11
PSET (60, 50), 12
CLS
LINE (0, 0)-(500, 500), 14
VIEW
sum& = 0
FOR y = 0 TO 199
  FOR x = 0 TO 319
    sum& = sum& + POINT(x, y) * ((x + 3 * y) MOD 7 + 1)
  NEXT
NEXT
PRINT #1, sum&
CLOSE #1

SUB dump
  FOR y = 0 TO 80 STEP 3
    FOR x = 0 TO 110 STEP 2
      PRINT #1, POINT(x, y);
    NEXT
    PRINT #1,
  NEXT
END SUB
