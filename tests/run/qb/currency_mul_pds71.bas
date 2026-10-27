' CURRENCY * over positive, negative, tie and near-overflow pairs: the quotient is rounded to even or, beyond 63 bits, the minimum.
' dialect: pds71
' flags: -O2 --isel=dag
DIM a AS CURRENCY, b AS CURRENCY, c AS CURRENCY
DATA 12345.6789, 98765.4321, -12345.6789, 98765.4321, 0.0001, 0.5, -0.0001, 0.5, 0.0003, 0.5, 0.0003, -0.5
DATA 3, 3, 0.3333, 3, 0.6667, 0.6667, 1234567, 7654321, 3037000, 3037000, 3037001, 3037001, 300000000, 300000000
DATA 922337203685477, 1, 922337203685477, -1, -922337203685477, 1.0001, 0.00005, 1, 99999999999, 0.00999
FOR i = 1 TO 18
  READ x#, y#
  a = x#: b = y#
  c = a * b
  PRINT i; c
NEXT
