' Recursive fib (all call overhead) + digits (accumulator starts at zero); result = fib(24) + sum of digit sums of 1..30000.
' Frames: VBDOS calls B$ENRA/B$EXSA per call, QB zeroes locals inline; C and Nib use plain stack frames and initialise explicitly.
DEFINT A-Z
DECLARE FUNCTION fib& (n AS INTEGER)
DECLARE FUNCTION digits% (v AS LONG)
DECLARE FUNCTION BenchFrames& ()

PRINT LTRIM$(STR$(BenchFrames&))

FUNCTION BenchFrames&
    DIM sum AS LONG, i AS LONG
    FOR i = 1 TO 30000
        sum = sum + digits%(i)
    NEXT
    BenchFrames& = fib&(24) + sum
END FUNCTION

FUNCTION fib& (n AS INTEGER)
    DIM a AS LONG, b AS LONG, m AS INTEGER
    IF n < 2 THEN
        fib& = n
        EXIT FUNCTION
    END IF
    m = n - 1
    a = fib&(m)
    m = n - 2
    b = fib&(m)
    fib& = a + b
END FUNCTION

FUNCTION digits% (v AS LONG)
    DIM total AS INTEGER, rest AS LONG
    rest = v
    DO WHILE rest > 0
        total = total + rest MOD 10
        rest = rest \ 10
    LOOP
    digits% = total
END FUNCTION
