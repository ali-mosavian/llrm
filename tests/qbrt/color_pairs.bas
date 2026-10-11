' Which pairs of numbers COLOR takes in each graphics mode: a matrix of accepted (1) and refused (0), the first
' number down and the second across, then the colour a PSET without a colour draws after the pair that was taken.
DIM modes%(1 TO 7), v%(1 TO 11)
FOR i% = 1 TO 7: READ modes%(i%): NEXT
FOR i% = 1 TO 11: READ v%(i%): NEXT
DATA 1,2,7,8,9,12,13
DATA 0,1,2,3,7,15,16,31,32,255,256
ON ERROR GOTO bad
FOR i% = 1 TO 7
  SCREEN modes%(i%)
  m$ = ""
  FOR a% = 1 TO 11
    FOR b% = 1 TO 11
      failed% = 0
      COLOR v%(a%), v%(b%)
      m$ = m$ + MID$("10", failed% + 1, 1)
    NEXT
    m$ = m$ + "/"
  NEXT
  SCREEN 0
  PRINT "mode"; modes%(i%)
  PRINT m$
NEXT
END
bad:
failed% = 1
RESUME NEXT
