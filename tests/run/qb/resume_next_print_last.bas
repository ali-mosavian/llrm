' flags: -O0 | -O2
' RESUME NEXT after an error in a PRINT that ends a FOR body runs the next iteration.
ON ERROR GOTO h
FOR k% = 1 TO 3: PRINT 10 \ (k% - 1): NEXT
PRINT "done"
j% = 0
WHILE j% < 3
  j% = j% + 1
  PRINT 10 \ (j% - 2)
WEND
PRINT "w done"
i% = 0
DO
  i% = i% + 1
  PRINT 10 \ (i% - 2)
LOOP WHILE i% < 3
PRINT "d done"
n% = 0
FOR k% = 1 TO 3
  PRINT "k"; k%
  n% = n% + 10 \ (k% - 2)
NEXT
PRINT "a done"; n%
END
h: PRINT "err"; ERR: RESUME NEXT
