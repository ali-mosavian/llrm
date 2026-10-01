' A huge array -- over 64K, so /AH, so every element goes through the
' runtime. n is a LONG: 40000 in an INTEGER is a Math overflow in BC and in
' llrm. Not in suite/: PDS refuses the REDIM with a math overflow, and the
' matrix compiles every suite program on every configuration.
'
'   BC /O /AH HUGE.BAS, HUGE.OBJ;
DEFINT A-Z
DIM n AS LONG, i, t
n = 40000
REDIM h(n)
t = 0
FOR i = 1 TO 10
    h(i * 1000) = i
    t = t + h(i * 1000)
NEXT i
PRINT "T="; t
PRINT "DONE"
