' Towers of Hanoi with 13 discs: the number of moves.
DEFINT A-Z
DECLARE FUNCTION Hanoi% (n AS INTEGER, a AS INTEGER, b AS INTEGER, c AS INTEGER)
DECLARE FUNCTION BenchHanoi& (n AS INTEGER)

PRINT LTRIM$(STR$(BenchHanoi&(13)))

FUNCTION Hanoi% (n AS INTEGER, a AS INTEGER, b AS INTEGER, c AS INTEGER)
    IF n = 0 THEN
        Hanoi% = 0
    ELSE
        Hanoi% = Hanoi%(n - 1, a, c, b) + 1 + Hanoi%(n - 1, c, b, a)
    END IF
END FUNCTION

FUNCTION BenchHanoi& (n AS INTEGER)
    BenchHanoi& = Hanoi%(n, 1, 3, 2)
END FUNCTION
