' CURRENCY + - * / and comparisons: ties round to even, + - overflow is error 6, * overflow is the minimum, / is DOUBLE.
' dialect: pds71
' flags: -O2 --isel=dag
DIM a AS CURRENCY, b AS CURRENCY, c AS CURRENCY
DIM i AS INTEGER, l AS LONG, d AS DOUBLE
ON ERROR GOTO h
a = 1.5: b = 2.25
c = a + b: PRINT c
c = a - b: PRINT c
c = a * b: PRINT c
c = a / b: PRINT c
a = 0.0001: b = 0.5: c = a * b: PRINT c
a = 0.0003: b = 0.5: c = a * b: PRINT c
a = -0.0001: b = 0.5: c = a * b: PRINT c
a = -0.0003: b = 0.5: c = a * b: PRINT c
a = 0.0001: b = 0.0001: c = a * b: PRINT c
a = 922337203685477.5807@: b = 1: c = a + b: PRINT c
a = 922337203685477.5807@: b = 2: c = a * b: PRINT c
a = 922337203685477.5807@: b = 1.0001@: c = a * b: PRINT c
a = -922337203685477.5807@: b = 1: c = a - b: PRINT c
a = 1000000: b = 1000000: c = a * b: PRINT c
a = 1000000: b = 10000000: c = a * b: PRINT c
a = 5: b = 0: c = a / b: PRINT c
PRINT a + 1; a * 2; a - 0.5; a / 2; a + 0.1#; a * i; a + l
PRINT a < b; a > b; a = b; a <> b; a <= b; a >= b
a = 1.5: b = 1.5: PRINT a < b; a = b; a >= b
i = 3: PRINT a < i; a > 1; a = 1.5; a < 1.6#; a < 1.4!
PRINT -a; a + -b
c = a + i
c = a * 2.5
PRINT c
c = 7 / 2: PRINT c
c = a ^ 2
END
h:
PRINT "err"; ERR
RESUME NEXT
