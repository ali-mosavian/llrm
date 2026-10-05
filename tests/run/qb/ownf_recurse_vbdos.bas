' dialect: vbdos
' Own frames: recursion keeps each call's locals apart.
DECLARE FUNCTION fib% (n AS INTEGER)
DECLARE FUNCTION depth& (n AS INTEGER)
DECLARE SUB down (n AS INTEGER, total AS LONG)
DEFINT A-Z
DIM t AS LONG
PRINT fib(15)
PRINT depth(30)
t = 0
down 20, t
PRINT t
END
FUNCTION fib% (n AS INTEGER)
IF n < 2 THEN fib% = n ELSE fib% = fib(n - 1) + fib(n - 2)
END FUNCTION
FUNCTION depth& (n AS INTEGER)
DIM mine AS LONG
mine = n
IF n = 0 THEN depth& = 0 ELSE depth& = depth(n - 1) + mine
END FUNCTION
SUB down (n AS INTEGER, total AS LONG)
DIM mine AS INTEGER
mine = n
IF n > 0 THEN down n - 1, total
total = total + mine
END SUB
