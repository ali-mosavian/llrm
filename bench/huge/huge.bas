' dialect: pds71
' flags: -O2 --cpu 486 --huge-arrays
' REDIM'd 200x201 INTEGER array, 80400 bytes: /AH, every element through the runtime (a 1D array over 32767 elements fails: #359).
DEFINT A-Z
DECLARE FUNCTION BenchHuge& ()
PRINT LTRIM$(STR$(BenchHuge&))
END

FUNCTION BenchHuge&
    DIM t AS LONG
    REDIM h(0 TO 199, 0 TO 200)
    FOR r = 0 TO 199
        FOR c = 0 TO 200
            h(r, c) = (r * 201& + c) MOD 251
        NEXT c
    NEXT r
    t = 0
    FOR r = 0 TO 199
        FOR c = 0 TO 200
            t = t + h(r, c)
        NEXT c
    NEXT r
    BenchHuge& = t
END FUNCTION
