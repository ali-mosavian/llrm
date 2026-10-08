' Solutions of the 7-queens problem, counted by recursive placement.
DEFINT A-Z
DECLARE FUNCTION Safe% (q() AS INTEGER, row AS INTEGER, col AS INTEGER)
DECLARE FUNCTION Place% (q() AS INTEGER, row AS INTEGER, n AS INTEGER)
DECLARE FUNCTION BenchQueens& (n AS INTEGER)

PRINT LTRIM$(STR$(BenchQueens&(7)))

FUNCTION Safe% (q() AS INTEGER, row AS INTEGER, col AS INTEGER)
    DIM r AS INTEGER, d AS INTEGER
    FOR r = 0 TO row - 1
        d = q(r) - col
        IF d = 0 OR d = row - r OR d = r - row THEN
            Safe% = 0
            EXIT FUNCTION
        END IF
    NEXT
    Safe% = -1
END FUNCTION

FUNCTION Place% (q() AS INTEGER, row AS INTEGER, n AS INTEGER)
    DIM col AS INTEGER, found AS INTEGER
    IF row = n THEN
        Place% = 1
        EXIT FUNCTION
    END IF
    found = 0
    FOR col = 0 TO n - 1
        IF Safe%(q(), row, col) THEN
            q(row) = col
            found = found + Place%(q(), row + 1, n)
        END IF
    NEXT
    Place% = found
END FUNCTION

FUNCTION BenchQueens& (n AS INTEGER)
    DIM q(0 TO 11) AS INTEGER
    BenchQueens& = Place%(q(), 0, n)
END FUNCTION
