' Backgrounds above 7 are the colours 0 to 7; foregrounds above 15 blink.
CLS
DEF SEG = &HB800
DIM a%(20)
FOR b% = 0 TO 15
  COLOR 7, b%
  LOCATE b% + 1, 1: PRINT "x";
  a%(b%) = PEEK(b% * 160 + 1)
NEXT
FOR f% = 16 TO 20
  COLOR f%, 1
  LOCATE f% - 15, 5: PRINT "y";
  a%(f%) = PEEK((f% - 16) * 160 + 9)
NEXT
COLOR 7, 0
LOCATE 1, 20
FOR i% = 0 TO 20: PRINT a%(i%);: NEXT
LOCATE 25, 1
