' Binary tree in an array (key, left, right per node; 0 is nil): recursive insert of 300 keys, recursive in-order walk.
DEFINT A-Z
DECLARE SUB AddNode (t() AS INTEGER, at AS INTEGER, node AS INTEGER)
DECLARE FUNCTION Walk& (t() AS INTEGER, at AS INTEGER, depth AS INTEGER)
DECLARE FUNCTION BenchBintree& (seed&)

PRINT LTRIM$(STR$(BenchBintree&(1)))

SUB AddNode (t() AS INTEGER, at AS INTEGER, node AS INTEGER)
    DIM side AS INTEGER
    IF t(3 * node) < t(3 * at) THEN side = 1 ELSE side = 2
    IF t(3 * at + side) = 0 THEN
        t(3 * at + side) = node
    ELSE
        AddNode t(), t(3 * at + side), node
    END IF
END SUB

FUNCTION Walk& (t() AS INTEGER, at AS INTEGER, depth AS INTEGER)
    IF at = 0 THEN
        Walk& = 0
    ELSE
        Walk& = Walk&(t(), t(3 * at + 1), depth + 1) + t(3 * at) * CLNG(depth) + Walk&(t(), t(3 * at + 2), depth + 1)
    END IF
END FUNCTION

FUNCTION BenchBintree& (seed&)
    DIM t(0 TO 902) AS INTEGER
    DIM i AS INTEGER
    DIM x AS LONG
    FOR i = 0 TO 902
        t(i) = 0
    NEXT
    x = seed&
    FOR i = 1 TO 300
        x = (x * 25173 + 13849) AND &HFFFF&
        t(3 * i) = x AND &HFFF
        IF i > 1 THEN AddNode t(), 1, i
    NEXT
    BenchBintree& = Walk&(t(), 1, 1)
END FUNCTION
