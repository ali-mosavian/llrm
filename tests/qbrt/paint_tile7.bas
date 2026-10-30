' PAINT with a tile string, up to a border: the tile's pixels as BC lays them.
DEFINT A-Z
OPEN "L.TXT" FOR OUTPUT AS #1
SCREEN 7
t$ = CHR$(&HAA) + CHR$(&H55) + CHR$(&HF0) + CHR$(&H0F) + CHR$(&H33) + CHR$(&HCC) + CHR$(0) + CHR$(&HFF)
LINE (20, 20)-(120, 90), 4, B
LINE (60, 40)-(80, 60), 4, B
PAINT (30, 30), t$, 4
CIRCLE (220, 100), 40, 4
PAINT (220, 100), t$, 4
FOR y = 28 TO 35
  FOR x = 28 TO 43
    PRINT #1, POINT(x, y);
  NEXT
  PRINT #1,
NEXT
sum& = 0
FOR y = 0 TO 199
  FOR x = 0 TO 319
    sum& = sum& + POINT(x, y) * ((x + 3 * y) MOD 7 + 1)
  NEXT
NEXT
PRINT #1, sum&; POINT(70, 50); POINT(220, 100)
CLOSE #1
