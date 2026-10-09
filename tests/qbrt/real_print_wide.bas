' Values from the denormals to the top of the range, and numbers one rounding
' step either side of a digit boundary, printed as SINGLE and DOUBLE.
DIM d AS DOUBLE, s AS SINGLE
d = 1D-300
FOR i% = 1 TO 90
    PRINT d, d * 3.3333#
    d = d * 17.3#
    IF d > 1D+300 THEN d = d / 1D+290
NEXT
d = 4.94D-324
FOR i% = 1 TO 12
    PRINT d: d = d * 100#
NEXT
FOR i% = 1 TO 40
    d = 1# + i% * 1D-16
    s = d
    PRINT d, s, d * 7.0000000000000007#, 1# / i%, i% / 7#
NEXT
FOR i% = -7 TO 7
    d = 99999999999999.95# * 10# ^ i%
    PRINT d, CSNG(d)
NEXT
