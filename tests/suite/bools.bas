' A comparison used as a value, and two used only to branch on. BC
' materialises every one into a register through a branch and a dec,
' then compares that register against zero to branch again.
DEFINT A-Z
DIM a, b, c, d, x, t
a = 3
b = 7
c = 2
d = 9
t = 0
x = (a < b)
t = t + x
IF a < b THEN t = t + 1
IF (a < b) AND (c < d) THEN t = t + 2
PRINT "T="; t
PRINT "DONE"
