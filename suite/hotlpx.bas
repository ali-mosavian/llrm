' hotlop, with n and k read at runtime so nothing can fold them. The
' product is still loop-invariant and still recomputed twenty times --
' which is the point: hoisting it is a different thing from folding it,
' and a program whose constants are visible cannot tell them apart.
DEFINT A-Z
DIM n, k, s, i
DATA 7, 3
READ n, k
s = 0
FOR i = 1 TO 20
    s = s + (n * k) + i
NEXT i
PRINT "S="; s
PRINT "DONE"
