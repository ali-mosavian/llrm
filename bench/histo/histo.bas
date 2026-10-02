' Mirrors histo.c; QB has no unsigned char, so the table is an INTEGER array holding 0..255.
DEFINT A-Z
DECLARE FUNCTION BenchHisto& (seed AS INTEGER)
DECLARE SUB Histogram (seed AS INTEGER)

DIM SHARED dat(0 TO 4095) AS INTEGER
DIM SHARED counts(0 TO 255) AS INTEGER

PRINT LTRIM$(STR$(BenchHisto&(3)))

SUB Histogram (seed AS INTEGER)
    FOR i = 0 TO 4095
        j = (dat(i) * 7 + seed) AND 255
        counts(j) = counts(j) + 1
    NEXT
END SUB

FUNCTION BenchHisto& (seed AS INTEGER)
    DIM sum AS LONG
    FOR i = 0 TO 4095
        dat(i) = ((i AND 127) * (i \ 32)) AND 255
    NEXT
    FOR i = 0 TO 255
        counts(i) = 0
    NEXT
    Histogram seed
    FOR i = 0 TO 255
        sum = sum + CLNG(counts(i)) * (i + 1)
    NEXT
    BenchHisto& = sum
END FUNCTION
