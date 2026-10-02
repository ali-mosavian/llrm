' dialect: pds71
' flags: -O2 --cpu 486 --huge-arrays
' 10000 pairs of LONGs (80000 bytes) REDIM'd /AH: 8-byte elements.
DEFINT A-Z
TYPE Pair
    x AS LONG
    y AS LONG
END TYPE
DECLARE FUNCTION BenchPairs& ()
PRINT LTRIM$(STR$(BenchPairs&))
END

FUNCTION BenchPairs&
    DIM t AS LONG
    REDIM p(0 TO 9999) AS Pair
    FOR i = 0 TO 9999
        p(i).x = i
        p(i).y = i * 3&
    NEXT i
    t = 0
    FOR i = 0 TO 9999
        t = t + p(i).y - p(i).x
    NEXT i
    BenchPairs& = t
END FUNCTION
