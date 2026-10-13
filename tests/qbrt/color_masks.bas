' What a colour number out of a mode's range draws, as POINT reads it back, for PSET, LINE and PAINT's
' fill, and what COLOR then makes of the default colour.
DIM modes%(1 TO 7), samples%(1 TO 16)
FOR i% = 1 TO 7: READ modes%(i%): NEXT
FOR i% = 1 TO 16: READ samples%(i%): NEXT
DATA 1,2,7,8,9,12,13
DATA -2,-1,0,1,2,3,4,15,16,17,31,32,255,256,257,300
ON ERROR GOTO bad
FOR i% = 1 TO 7
  SCREEN modes%(i%)
  FOR j% = 1 TO 16
    c% = samples%(j%)
    PSET (1, 1), 0
    failed% = 0: PSET (1, 1), c%: a% = POINT(1, 1)
    LINE (10, 10)-(12, 10), 0
    failed% = 0: LINE (10, 10)-(12, 10), c%: b% = POINT(11, 10)
    LINE (20, 20)-(40, 40), 0, BF
    failed% = 0: PAINT (30, 30), c%, 254: d% = POINT(30, 30)
    r$ = r$ + STR$(a%) + "," + LTRIM$(STR$(b%)) + "," + LTRIM$(STR$(d%)) + ";"
  NEXT
  COLOR 1
  LINE (50, 50)-(52, 50)
  e% = POINT(51, 50)
  SCREEN 0
  PRINT "mode"; modes%(i%); " fg after COLOR 1:"; e%
  PRINT r$
  r$ = ""
NEXT
END
bad:
failed% = 1
r$ = r$ + "E"
RESUME NEXT
