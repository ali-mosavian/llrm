' The colour numbers each statement takes in each screen mode, as runs of accepted numbers: PSET, LINE,
' PAINT and COLOR.  The runtime took only what the first modes needed.
DIM ok%(-2 TO 300, 1 TO 4)
DIM modes%(1 TO 7)
FOR i% = 1 TO 7
  READ modes%(i%)
NEXT
DATA 1,2,7,8,9,12,13
ON ERROR GOTO bad
FOR i% = 1 TO 7
  SCREEN modes%(i%)
  FOR c% = -2 TO 300
    failed% = 0: PSET (1, 1), c%: ok%(c%, 1) = 1 - failed%
    failed% = 0: LINE (1, 1)-(2, 2), c%: ok%(c%, 2) = 1 - failed%
    failed% = 0: PAINT (1, 5), c%, 255: ok%(c%, 3) = 1 - failed%
    failed% = 0: COLOR c%: ok%(c%, 4) = 1 - failed%
  NEXT
  COLOR 1
  SCREEN 0
  PRINT "mode"; modes%(i%)
  FOR k% = 1 TO 4
    s$ = ""
    c% = -2
    DO WHILE c% <= 300
      IF ok%(c%, k%) = 1 THEN
        a% = c%
        DO WHILE c% < 300 AND ok%(c% + 1, k%) = 1
          c% = c% + 1
        LOOP
        s$ = s$ + STR$(a%) + "-" + LTRIM$(STR$(c%))
      END IF
      c% = c% + 1
    LOOP
    PRINT k%; s$
  NEXT
NEXT
END
bad:
failed% = 1
RESUME NEXT
