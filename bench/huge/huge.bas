' dialect: pds71
' flags: -O2 -march=i486 --huge-arrays
' REDIM'd 201x200 INTEGER array, 80400 bytes, /AH. Column-major, so h(c, r) walks memory in C's order.
DEFINT A-Z
DECLARE FUNCTION BenchHuge& ()
PRINT LTRIM$(STR$(BenchHuge&))
END

FUNCTION BenchHuge&
    DIM t AS LONG
    REDIM h(0 TO 200, 0 TO 199)
    FOR r = 0 TO 199
        FOR c = 0 TO 200
            h(c, r) = (r * 201& + c) MOD 251
        NEXT c
    NEXT r
    t = 0
    FOR r = 0 TO 199
        FOR c = 0 TO 200
            t = t + h(c, r)
        NEXT c
    NEXT r
    BenchHuge& = t
END FUNCTION
