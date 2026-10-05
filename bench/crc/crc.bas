' No unsigned LONG: logical shift is ((crc AND &HFFFFFFFE) \ 2) AND &H7FFFFFFF; result prints signed.
DEFINT A-Z
DECLARE FUNCTION BenchCrc& (salt AS LONG)

PRINT LTRIM$(STR$(BenchCrc&(0)))

FUNCTION BenchCrc& (salt AS LONG)
    DIM crc AS LONG
    d$ = "123456789"
    crc = &HFFFFFFFF XOR salt
    FOR i = 0 TO 8
        crc = crc XOR ASC(MID$(d$, i + 1, 1))
        FOR b = 0 TO 7
            crc = (((crc AND &HFFFFFFFE) \ 2) AND &H7FFFFFFF) XOR (&HEDB88320 AND (0& - (crc AND 1&)))
        NEXT b
    NEXT i
    BenchCrc& = crc XOR &HFFFFFFFF
END FUNCTION
