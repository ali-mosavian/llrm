' BASIC has no unsigned types: the 16-bit values are held in LONGs (0..65535).
DEFINT A-Z
DECLARE FUNCTION BenchShellsort& (seed&)

PRINT LTRIM$(STR$(BenchShellsort&(0)))

FUNCTION BenchShellsort& (seed&)
    DIM values(0 TO 63) AS LONG
    DIM i AS INTEGER, gap AS INTEGER, at AS INTEGER
    DIM value AS LONG, checksum AS LONG
    FOR i = 0 TO 63
        values(i) = ((CLNG(i) * 109 + 37) XOR (CLNG(i) * 128) XOR seed&) AND &HFFFF&
    NEXT
    gap = 32
    DO WHILE gap <> 0
        FOR i = gap TO 63
            at = i
            value = values(i)
            DO WHILE at >= gap
                IF values(at - gap) <= value THEN EXIT DO
                values(at) = values(at - gap)
                at = at - gap
            LOOP
            values(at) = value
        NEXT
        gap = gap \ 2
    LOOP
    checksum = 0
    FOR i = 0 TO 63
        checksum = checksum + values(i) * (i + 1)
    NEXT
    BenchShellsort& = checksum
END FUNCTION
