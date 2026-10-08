' dialect: pds71
' flags: -O2 -march=i486 --huge-arrays
' 40000 INTEGERs (80000 bytes) in huge memory, past 64K (a subscript is at most 32767, so each array is two columns wide: a(k AND 1, k \ 2) is word k): copied, scrolled up and scrolled down; the kernel returns the sum of the three weighted sums.
DEFINT A-Z
DECLARE FUNCTION Weigh& ()
DECLARE FUNCTION BenchCopyw& ()
REDIM SHARED a(0 TO 1, 0 TO 19999) AS INTEGER
REDIM SHARED b(0 TO 1, 0 TO 19999) AS INTEGER
PRINT LTRIM$(STR$(BenchCopyw&))
END

FUNCTION BenchCopyw&
    DIM t AS LONG, i AS LONG, j AS LONG
    FOR i = 0 TO 39999
        a(i AND 1, i \ 2) = ((i * 3 + 1 + 32768) AND 65535) - 32768
    NEXT i
    FOR i = 0 TO 39999
        b(i AND 1, i \ 2) = a(i AND 1, i \ 2)
    NEXT i
    t = Weigh&
    FOR i = 0 TO 38999
        b(i AND 1, i \ 2) = b((i + 1000) AND 1, (i + 1000) \ 2)
    NEXT i
    t = t + Weigh&
    FOR j = 38999 TO 0 STEP -1
        b((j + 1000) AND 1, (j + 1000) \ 2) = b(j AND 1, j \ 2)
    NEXT j
    BenchCopyw& = t + Weigh&
END FUNCTION

FUNCTION Weigh&
    DIM t AS LONG, i AS LONG
    FOR i = 0 TO 39999
        t = t + b(i AND 1, i \ 2) * ((i AND 15) + 1&)
    NEXT i
    Weigh& = t
END FUNCTION
