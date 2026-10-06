' dialect: pds71
' flags: -O2 -march=i486 --huge-arrays
' 201x100 LONGs (80400 bytes) REDIM'd /AH, walked in memory order: column-major, so g(c, r).
DEFINT A-Z
DECLARE FUNCTION BenchGrid2d& ()
PRINT LTRIM$(STR$(BenchGrid2d&))
END

FUNCTION BenchGrid2d&
    DIM t AS LONG
    REDIM g(0 TO 200, 0 TO 99) AS LONG
    FOR r = 0 TO 99
        FOR c = 0 TO 200
            g(c, r) = r * 1000& + c
        NEXT c
    NEXT r
    t = 0
    FOR r = 0 TO 99
        FOR c = 0 TO 200
            t = t + (g(c, r) AND 255)
        NEXT c
    NEXT r
    BenchGrid2d& = t
END FUNCTION
