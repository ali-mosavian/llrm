' Prime count and sum below 1024 in one checkable word; mirrors sieve.c.
' QB has no unsigned types: count and i are INTEGER (fit in 15 bits), sum is LONG.
DEFINT A-Z
DECLARE FUNCTION BenchSieve& (limit AS INTEGER)

PRINT LTRIM$(STR$(BenchSieve&(1024)))

FUNCTION BenchSieve& (limit AS INTEGER)
    DIM composite(0 TO 1023) AS INTEGER
    DIM i AS INTEGER, multiple AS INTEGER, count AS INTEGER
    DIM sum AS LONG
    FOR i = 0 TO limit - 1
        composite(i) = 0
    NEXT
    FOR i = 2 TO limit - 1
        IF composite(i) = 0 THEN
            count = count + 1
            sum = sum + i
            IF i <= 31 THEN
                multiple = i * i
                DO WHILE multiple < limit
                    composite(multiple) = 1
                    multiple = multiple + i
                LOOP
            END IF
        END IF
    NEXT
    BenchSieve& = (CLNG(count) * 65536) XOR sum
END FUNCTION
