' flags: -O1 | -O2 | -Os
' dialect: qb45 pds71
DEFINT A-Z
DIM SHARED z AS INTEGER, w AS INTEGER
w = 1
FOR round = 1 TO 3
  z = 21000 + 257 * round
  hi = z \ 256: lo = z AND 255
  bse = VARPTR(w) - 1024
  IF bse < 0 THEN bse = 0
  sg = VARSEG(w) + 0
  DEF SEG = sg
  found = -1
  FOR o = bse TO bse + 2048
    IF PEEK(o) = lo AND PEEK(o + 1) = hi THEN found = o: EXIT FOR
  NEXT o
  IF found >= 0 THEN
    POKE found, 99 + round
    POKE found + 1, 0
  END IF
  DEF SEG
  PRINT round; found >= 0; z
NEXT round
PRINT z
