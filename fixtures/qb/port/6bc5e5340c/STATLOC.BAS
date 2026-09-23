DECLARE FUNCTION F& ()
DIM total AS LONG
total = F&
PRINT total
FUNCTION F& STATIC
DIM total AS LONG
total = 3
F& = total
END FUNCTION
