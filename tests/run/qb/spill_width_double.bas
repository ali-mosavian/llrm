' flags: -O1 | -O2 | -O3 | -Os
' dialect: qb45
' 10# ^ i% over a 15-trip loop, unrolled at -O3: log2(10) spilled as a qword made copy 1 (register, 80 bits) differ from the rest (D-RFP5).
DIM d AS DOUBLE
FOR i% = -7 TO 7
    d = 99999999999999.95# * 10# ^ i%
    PRINT d
NEXT
