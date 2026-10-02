' dialect: pds71
' flags: -O2 --cpu 486 --huge-arrays
' 30000 LONGs (120000 bytes) REDIM'd /AH, summed from the top down.
DEFINT A-Z
DECLARE FUNCTION BenchDown1d& ()
PRINT LTRIM$(STR$(BenchDown1d&))
END

FUNCTION BenchDown1d&
    DIM t AS LONG
    REDIM a(0 TO 29999) AS LONG
    FOR i = 0 TO 29999
        a(i) = i
    NEXT i
    t = 0
    FOR i = 29999 TO 0 STEP -1
        t = (t * 3 + a(i)) AND 65535
    NEXT i
    BenchDown1d& = t
END FUNCTION
