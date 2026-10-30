' flags: -O1 | -O2 | -O3 | -Os
' 10# ^ i% over a 15-trip loop, unrolled at -O3: the first copy once used the 80-bit log2(10) the others had stored as a double (real_print_wide).
DIM d AS DOUBLE
FOR i% = -7 TO 7
    d = 99999999999999.95# * 10# ^ i%
    PRINT d
NEXT
