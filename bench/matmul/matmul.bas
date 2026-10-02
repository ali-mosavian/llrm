' No unsigned types: the checksum is LONG (it stays far below 2^31).
DEFINT I-K
DECLARE FUNCTION BenchMatmul& (seed AS INTEGER)

PRINT LTRIM$(STR$(BenchMatmul&(0)))
END

FUNCTION BenchMatmul& (seed AS INTEGER)
    DIM a(7, 7) AS INTEGER, b(7, 7) AS INTEGER
    DIM c(7, 7) AS LONG
    DIM i AS INTEGER, j AS INTEGER, k AS INTEGER
    DIM total AS LONG, checksum AS LONG

    FOR i = 0 TO 7
        FOR j = 0 TO 7
            a(i, j) = i * 3 + j + 1 + seed
            IF i = j THEN b(i, j) = 2 ELSE b(i, j) = (i + j) MOD 3
        NEXT j
    NEXT i
    FOR i = 0 TO 7
        FOR j = 0 TO 7
            total = 0
            FOR k = 0 TO 7
                total = total + CLNG(a(i, k)) * b(k, j)
            NEXT k
            c(i, j) = total
        NEXT j
    NEXT i
    FOR i = 0 TO 7
        FOR j = 0 TO 7
            checksum = checksum + c(i, j) * (i * 8 + j + 1)
        NEXT j
    NEXT i
    BenchMatmul& = checksum
END FUNCTION
