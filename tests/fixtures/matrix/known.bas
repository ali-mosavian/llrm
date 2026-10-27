DECLARE FUNCTION Add% (first AS INTEGER, snd AS INTEGER)
DIM total AS INTEGER
total = Add%(3, 4)
PRINT total
FUNCTION Add% (first AS INTEGER, snd AS INTEGER)
    DIM sum AS INTEGER
    sum = first + snd
    Add% = sum
END FUNCTION
