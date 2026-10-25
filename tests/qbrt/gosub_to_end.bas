' A GOSUB to a label that ends the program (no RETURN below it) is a jump.
DEFINT A-Z
FOR q = 1 TO 3
  ON q GOSUB 100, 200, 300
  PRINT "back"; q
NEXT
100 PRINT "one": RETURN
200 PRINT "two": RETURN
300 PRINT "quit"
END
