DECLARE FUNCTION Add% (a AS INTEGER, b AS INTEGER)
DIM total AS INTEGER
total = Add%(3, 4)
PRINT total
FUNCTION Add% (a AS INTEGER, b AS INTEGER)
    DIM sum AS INTEGER
    sum = a + b
    Add% = sum
END FUNCTION
