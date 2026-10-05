' DOUBLE variable is stored to memory each statement (no volatile needed); INTEGER counter (no unsigned).
DECLARE FUNCTION BenchFloats& (iterations AS INTEGER)

PRINT LTRIM$(STR$(BenchFloats&(1000)))

FUNCTION BenchFloats& (iterations AS INTEGER)
    DIM value AS DOUBLE
    DIM i AS INTEGER
    value = 1#
    FOR i = 0 TO iterations - 1
        value = (value * 1.0009765625# + .125#) / 1.00048828125#
    NEXT i
    BenchFloats& = CLNG(FIX(value * 1000#))
END FUNCTION
