DECLARE FUNCTION Add% (first AS INTEGER, second AS INTEGER)
DIM total AS INTEGER
total = Add%(3, 4)
PRINT total
FUNCTION Add% (first AS INTEGER, second AS INTEGER)
    DIM sum AS INTEGER
    sum = first + second
    Add% = sum
END FUNCTION
