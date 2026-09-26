' lngmix, with the dividend read at runtime. The divide and the remainder
' are still invariant and still two runtime calls on every pass.
DEFINT A-Z
DIM v AS LONG, s AS LONG
DIM i
DATA 100000
READ v
s = 0
FOR i = 1 TO 10
    s = s + v \ 7 + v MOD 7
NEXT i
PRINT "S="; s
PRINT "DONE"
