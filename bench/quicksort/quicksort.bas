' Recursive quicksort (Lomuto) of 1024 values from a 16-bit LCG; the checksum is -1 if the result is not sorted.
DEFINT A-Z
DECLARE SUB Sort (a() AS INTEGER, lo AS INTEGER, hi AS INTEGER)
DECLARE FUNCTION BenchQuicksort& (seed&)

PRINT LTRIM$(STR$(BenchQuicksort&(1)))

SUB Sort (a() AS INTEGER, lo AS INTEGER, hi AS INTEGER)
    DIM pivot AS INTEGER, i AS INTEGER, j AS INTEGER, t AS INTEGER
    IF lo >= hi THEN EXIT SUB
    pivot = a(hi)
    i = lo
    FOR j = lo TO hi - 1
        IF a(j) < pivot THEN
            t = a(i): a(i) = a(j): a(j) = t
            i = i + 1
        END IF
    NEXT
    t = a(i): a(i) = a(hi): a(hi) = t
    Sort a(), lo, i - 1
    Sort a(), i + 1, hi
END SUB

FUNCTION BenchQuicksort& (seed&)
    DIM values(0 TO 1023) AS INTEGER
    DIM i AS INTEGER
    DIM x AS LONG, checksum AS LONG
    x = seed&
    FOR i = 0 TO 1023
        x = (x * 25173 + 13849) AND &HFFFF&
        values(i) = x AND &H7FFF
    NEXT
    Sort values(), 0, 1023
    FOR i = 1 TO 1023
        IF values(i - 1) > values(i) THEN
            BenchQuicksort& = -1
            EXIT FUNCTION
        END IF
    NEXT
    checksum = 0
    FOR i = 0 TO 1023
        checksum = checksum + values(i) * CLNG((i AND 15) + 1)
    NEXT
    BenchQuicksort& = checksum
END FUNCTION
