' dialect: pds71
' flags: -O2 -march=i486 --huge-arrays
' Two REDIM'd arrays of 20000 LONGs (80000 bytes each), /AH: one read into the other.
DEFINT A-Z
DECLARE FUNCTION BenchCopy1d& ()
PRINT LTRIM$(STR$(BenchCopy1d&))
END

FUNCTION BenchCopy1d&
    DIM t AS LONG
    REDIM a(0 TO 19999) AS LONG
    REDIM b(0 TO 19999) AS LONG
    FOR i = 0 TO 19999
        a(i) = i
    NEXT i
    FOR i = 0 TO 19999
        b(i) = a(i) * 2 + 1
    NEXT i
    t = 0
    FOR i = 0 TO 19999
        t = t + b(i)
    NEXT i
    BenchCopy1d& = t
END FUNCTION
