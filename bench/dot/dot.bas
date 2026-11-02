' Arrays are module level; the count is the parameter.
DEFINT A-Z
DECLARE FUNCTION Dot& (n AS INTEGER)
DECLARE FUNCTION BenchDot& ()
DIM SHARED va(63), vb(63)

PRINT LTRIM$(STR$(BenchDot&))

FUNCTION BenchDot&
    FOR i = 0 TO 63
        va(i) = 3 * i - 50
        vb(i) = 100 - 5 * i
    NEXT i
    BenchDot& = Dot&(64)
END FUNCTION

FUNCTION Dot& (n AS INTEGER)
    t& = 0
    FOR i = 0 TO n - 1
        t& = t& + CLNG(va(i)) * vb(i)
    NEXT i
    Dot& = t&
END FUNCTION
