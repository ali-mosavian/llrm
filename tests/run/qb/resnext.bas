' RESUME NEXT after an error raised in a FUNCTION called from a SUB: it continues in the
' FUNCTION, after the failing statement (the divide traps; ERROR 11 is raised in code).
DECLARE SUB caller (d AS INTEGER)
DECLARE FUNCTION inner% (d AS INTEGER)
ON ERROR GOTO h
caller 0
caller 2
PRINT "done"
END
h:
PRINT "err"; ERR; ERL
RESUME NEXT
SUB caller (d AS INTEGER)
DIM a AS LONG, b AS INTEGER
900 a = 100
905 b = inner(d)
908 PRINT "caller"; a; b
END SUB
FUNCTION inner% (d AS INTEGER)
DIM c AS INTEGER
910 IF d = 0 THEN ERROR 11
915 c = 10 \ d
920 inner% = c + 1
END FUNCTION
