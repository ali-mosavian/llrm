' Procedure entry and exit, timed with RDTSC: a recursive FIB& that is almost
' all call overhead, and DIGITS%, whose accumulator starts at zero. Under
' VBDOS every call runs B$ENRA and B$EXSA; QuickrBASIC frames and zeroes
' inline. Every variable is declared, so both dialects compile it.
DECLARE SUB TscSnap (hi AS LONG, lo AS LONG)
DECLARE FUNCTION fib& (n AS INTEGER)
DECLARE FUNCTION digits% (v AS LONG)

DIM hi0 AS LONG, lo0 AS LONG, hi1 AS LONG, lo1 AS LONG, hi2 AS LONG, lo2 AS LONG
DIM fibs AS LONG, sum AS LONG, i AS LONG, n AS INTEGER

n = 24
CALL TscSnap(hi0, lo0)
fibs = fib&(n)
CALL TscSnap(hi1, lo1)
FOR i = 1 TO 30000
    sum = sum + digits%(i)
NEXT
CALL TscSnap(hi2, lo2)
PRINT "FIB="; fibs
PRINT "DIGITS="; sum
PRINT "TSC0="; hi0; lo0
PRINT "TSC1="; hi1; lo1
PRINT "TSC2="; hi2; lo2

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
