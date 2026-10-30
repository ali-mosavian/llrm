' flags: -O1 -fexcess-precision=standard | -O2 -fexcess-precision=standard | -O3 -fexcess-precision=standard | -Os -fexcess-precision=standard
' dialect: qb45
' Six doubles live across `10# ^ (i% - 5)`: the fexp2 bytes push two slots above their operand and overflowed the x87 stack, which reset the LSR double counter (b printed 40.04, not 44.44).
DIM a AS DOUBLE, b AS DOUBLE, c AS DOUBLE, d AS DOUBLE, e AS DOUBLE, f AS DOUBLE, g AS DOUBLE, h AS DOUBLE, p AS DOUBLE, q AS DOUBLE, r AS DOUBLE
a = 1.1#: b = 2.2#: c = 3.3#: d = 4.4#: e = 5.5#: f = 6.6#: g = 7.7#: h = 8.8#: p = 9.9#: q = 10.1#: r = 11.11#
FOR i% = 1 TO 9
    a = a * 10# ^ (i% - 5) + b
    b = b + (c + i%) * d
    p = p + d * e * 1D-6
    q = q * 1.0001# + g
    r = r + h * 1D-3
    PRINT a, b, c: PRINT d, e, f: PRINT g, h, p, q, r
NEXT
