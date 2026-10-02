' A long divide and remainder by the same invariant divisor, inside a
' loop. Both are runtime calls BC makes ten times over operands that
' never change.
DEFINT A-Z
DIM v AS LONG, s AS LONG
DIM i
s = 0
v = 100000
FOR i = 1 TO 10
    s = s + v \ 7 + v MOD 7
NEXT i
PRINT "S="; s
PRINT "DONE"
