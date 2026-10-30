' flags: -O1 | -O2 | -Os
' dialect: qb45 pds71
DEFINT A-Z
DIM SHARED x AS INTEGER, y AS LONG
x = 0
DEF SEG = VARSEG(x)
FOR i = 0 TO 5
  POKE VARPTR(x), (i * 37) AND 255
  POKE VARPTR(x) + 1, (i * 11) AND 127
  y = y + x
NEXT i
DEF SEG
PRINT x; y
