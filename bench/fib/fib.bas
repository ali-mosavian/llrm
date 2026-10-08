' Naive doubly recursive fib(20).
DEFINT A-Z
DECLARE FUNCTION Fib% (n AS INTEGER)
DECLARE FUNCTION BenchFib& (n AS INTEGER)

PRINT LTRIM$(STR$(BenchFib&(20)))

FUNCTION Fib% (n AS INTEGER)
    IF n < 2 THEN
        Fib% = n
    ELSE
        Fib% = Fib%(n - 1) + Fib%(n - 2)
    END IF
END FUNCTION

FUNCTION BenchFib& (n AS INTEGER)
    BenchFib& = Fib%(n)
END FUNCTION
