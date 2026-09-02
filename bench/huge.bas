' A huge array -- over 64K, so /AH, so every element goes through the
' runtime. Not in suite/ because only QuickBASIC 4.5 accepts this form:
' PDS refuses the REDIM with a math overflow, and the matrix compiles
' every suite program on every configuration.
'
'   BC /O /AH HUGE.BAS, HUGE.OBJ;
DEFINT A-Z
DIM n, i, t
n = 40000
REDIM h(n)
t = 0
FOR i = 1 TO 10
    h(i * 1000) = i
    t = t + h(i * 1000)
NEXT i
PRINT "T="; t
PRINT "DONE"
