' RESUME 0 is RESUME: the handler removes the cause (a zero divisor) and the statement runs again.
DEFINT A-Z
DIM SHARED den AS INTEGER
ON ERROR GOTO h
r = 10 \ den
PRINT "r"; r
END
h:
PRINT "err"; ERR
den = 2
RESUME 0
