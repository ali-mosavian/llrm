' dialect: pds71
' flags: -O2 --cpu 486 --huge-arrays
' 30000 LONGs (120000 bytes) REDIM'd /AH: fill, then sum.
DEFINT A-Z
DECLARE FUNCTION BenchLong1d& ()
PRINT LTRIM$(STR$(BenchLong1d&))
END

FUNCTION BenchLong1d&
    DIM t AS LONG
    REDIM a(0 TO 29999) AS LONG
    FOR i = 0 TO 29999
        a(i) = i + 5&
    NEXT i
    t = 0
    FOR i = 0 TO 29999
        t = t + a(i)
    NEXT i
    BenchLong1d& = t
END FUNCTION
