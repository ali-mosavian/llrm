' flags: -O1 | -O2 | -O3 | -Os
' dialect: qb45
' LOG of zero or a negative and SIN, COS, TAN of |x| above 2^31 raise Illegal function call (error 5); SQR of a negative is not an error (real BC 4.5).
ON ERROR GOTO h
DIM x AS DOUBLE, z AS DOUBLE
RESTORE d1
FOR k% = 1 TO 6
    READ x
    PRINT x;
    z = LOG(x)
    PRINT "log ok";
    z = SQR(x)
    PRINT "sqr ok"
    PRINT "."
NEXT
RESTORE d2
FOR k% = 1 TO 7
    READ x
    PRINT x;
    z = SIN(x)
    PRINT "sin ok";
    z = COS(x)
    PRINT "cos ok";
    z = TAN(x)
    PRINT "tan ok"
    PRINT "."
NEXT
END
h:
PRINT "error"; ERR
RESUME NEXT
d1: DATA 0,-1,-1D-300,1D-300,1,1D+300
d2: DATA 100,2147483648,2147483648.5,2147483649,-2147483648,-2147483649,1D+300
