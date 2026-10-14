' A value stored to a SINGLE or DOUBLE variable is rounded to that type, as BC does by storing through memory: a loop that
' accumulates one reads 1.280029E+08 (not the exact 1.28008E+08) in BC; llrm-qb -O1/-O2 kept the variable in an x87 register
' at extended precision.
OPEN "L.TXT" FOR OUTPUT AS #1
DIM a(16000) AS INTEGER
FOR i = 0 TO 16000: a(i) = i: NEXT
s = 0
FOR i = 0 TO 16000
  s = s + a(i)
NEXT
PRINT #1, s
d# = 0
FOR i = 1 TO 20000
  d# = d# + .1#
NEXT
PRINT #1, d#
t! = 0
FOR i = 1 TO 5000
  t! = t! * .999 + .1 / i
NEXT
PRINT #1, t!
CLOSE #1
