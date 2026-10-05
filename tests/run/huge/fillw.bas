' dialect: pds71
' flags: -O2 --cpu 486 --huge-arrays
' 40000 INTEGERs (80000 bytes) in huge memory filled with one value no one byte repeats: past 64K.
' A subscript is at most 32767, so the array is two columns wide: a(k AND 1, k \ 2) is word k of the run.
DEFINT A-Z
DECLARE FUNCTION BenchFillw& ()
PRINT LTRIM$(STR$(BenchFillw&))
END

FUNCTION BenchFillw&
    DIM t AS LONG, i AS LONG
    REDIM a(0 TO 1, 0 TO 19999) AS INTEGER
    FOR i = 0 TO 39999
        a(i AND 1, i \ 2) = 4660
    NEXT i
    FOR i = 0 TO 39999
        t = t + a(i AND 1, i \ 2)
    NEXT i
    BenchFillw& = t
END FUNCTION
