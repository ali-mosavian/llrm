' CURRENCY conversions to and from INTEGER, LONG, SINGLE and DOUBLE: ties round to even, overflow is silent.
' dialect: pds71
' flags: -O2 --isel=dag
DIM a AS CURRENCY, c AS CURRENCY
DIM i AS INTEGER, l AS LONG, d AS DOUBLE, s AS SINGLE
DATA .5, 1.5, 2.5, 3.5, -.5, -1.5, -2.5, .4999, .5001, 32767.4999, 32767.5, -32768.5, -32768.5001
DATA 100000.5, 2147483647.4999, 2147483647.5, -2147483648.5, -2147483649
ON ERROR GOTO h
FOR n = 1 TO 18
  READ x#
  c = x#
  PRINT x#; "=>"; c;
  i = c: PRINT " i="; i;
  l = c: PRINT " l="; l
NEXT
d = 922337203685477.5807#: PRINT d: c = d: PRINT c
d = 922337203685478#: c = d: PRINT c
d = -922337203685477.5808#: c = d: PRINT c
d = 0.00015#: c = d: PRINT c
d = 1D+30: c = d: PRINT c
s = 0.00005: c = s: PRINT c
s = 16777217: c = s: PRINT c
a = 1234567.1234: d = a: PRINT d: s = a: PRINT s
a = 922337203685477.5807@: d = a: PRINT d: s = a: PRINT s
a = .0001@: d = a: PRINT d
i = -32768: c = i: PRINT c
l = -2147483647: c = l: PRINT c
END
h:
PRINT "err"; ERR
RESUME NEXT
