' flags: -O1 | -O2 | -O3 | -Os
' dialect: qb45
' 10 ^ 308 and SINGLE 10 ^ 38 keep the 80-bit y * log2(x) through the range check: 1D+308 and 1E+38, not 9.999999999999683D+307 and 1.000001E+38 (real BC 4.5).
DIM x AS DOUBLE, y AS DOUBLE, s AS SINGLE, u AS SINGLE
x = 10: y = 308
PRINT x ^ y
s = 10: u = 38
PRINT s ^ u
x = 10: y = 22
PRINT x ^ y
x = 2: y = 0.5
PRINT x ^ y
