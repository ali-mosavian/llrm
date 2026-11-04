' Dot product of two arrays of different lengths, to the shorter, 40 times with the first element changed each time; Nib has no globals.
DEFINT A-Z
DECLARE FUNCTION Dot& (a() AS INTEGER, b() AS INTEGER)
DECLARE FUNCTION BenchDot& ()
PRINT LTRIM$(STR$(BenchDot&))
END

FUNCTION Dot& (a() AS INTEGER, b() AS INTEGER)
    DIM total AS LONG, i AS INTEGER, n AS INTEGER
    n = UBOUND(a)
    IF UBOUND(b) < n THEN n = UBOUND(b)
    FOR i = 0 TO n
        total = total + CLNG(a(i)) * b(i)
    NEXT
    Dot& = total
END FUNCTION

FUNCTION BenchDot&
    DIM a(0 TO 63) AS INTEGER, b(0 TO 47) AS INTEGER
    DIM i AS INTEGER, r AS INTEGER
    DIM s AS LONG
    FOR i = 0 TO 63: a(i) = i * 3 - 90: NEXT
    FOR i = 0 TO 47: b(i) = 70 - i * 2: NEXT
    FOR r = 0 TO 39
        a(0) = r
        s = s + Dot&(a(), b())
    NEXT
    BenchDot& = s
END FUNCTION
