' The numbers COLOR takes in each graphics mode as runs of accepted ones, for COLOR a, COLOR , b and COLOR a, 1.
DIM modes%(1 TO 7), ok%(0 TO 300, 1 TO 3)
FOR i% = 1 TO 7: READ modes%(i%): NEXT
DATA 1,2,7,8,9,12,13
ON ERROR GOTO bad
FOR i% = 1 TO 7
  SCREEN modes%(i%)
  FOR c% = 0 TO 300
    IF c% > 20 AND c% < 60 OR c% > 70 AND c% < 250 THEN c% = c% + 1: GOTO skip
    failed% = 0: COLOR c%: ok%(c%, 1) = 1 - failed%
    failed% = 0: COLOR , c%: ok%(c%, 2) = 1 - failed%
    failed% = 0: COLOR c%, 1: ok%(c%, 3) = 1 - failed%
skip:
  NEXT
  SCREEN 0
  PRINT "mode"; modes%(i%)
  FOR k% = 1 TO 3
    s$ = ""
    c% = 0
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
