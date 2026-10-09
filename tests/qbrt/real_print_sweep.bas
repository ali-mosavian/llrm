' Hundreds of SINGLE and DOUBLE values across the exponent range, printed:
' the digit extraction must agree with BCOM45's to the last place.
DIM d AS DOUBLE, e AS DOUBLE, s AS SINGLE
d = 1.2345678901234567#
e = 1#
FOR i% = 1 TO 120
    d = d * 1.37# + .0123456789#
    IF d > 1E+30 THEN d = d / 1.234567E+29
    e = e / 2.718281828#
    IF e < 1E-30 THEN e = e * 1E+28
    s = d
    PRINT d, e, s, s / 3
NEXT
FOR i% = -20 TO 20 STEP 4
    d = 1#
    FOR j% = 1 TO ABS(i%)
        IF i% > 0 THEN d = d * 10# ELSE d = d / 10#
    NEXT
    PRINT d, d * 7.5#, d / 7#, CSNG(d)
NEXT
