' RUN: llrm-qb %s --isel=dag -Os -m16 -S -o /dev/stdout
' A call whose contract lists SP among what it disturbs (B$DDIM, B$CEND: the
' callee pops its own arguments) made the body "move the stack", so the epilogue
' restored SP with `lea sp, [bp-N]` before the pops (3 bytes a function; 14 of
' the bench programs against the old selector).
' CHECK-LABEL: BENCHSIEVE proc
' CHECK-NOT: lea sp
' CHECK: retf 2
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
