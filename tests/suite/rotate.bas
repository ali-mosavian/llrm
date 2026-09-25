' Three loops: a constant trip count, a variable one, and one that never
' runs. BC jumps to the test on entry every time, and tests against a
' bound where counting down to zero would drop the compare.
DEFINT A-Z
DIM i, n, t
t = 0
n = 10
FOR i = 1 TO 10
    t = t + i
NEXT i
FOR i = 1 TO n
    t = t + i
NEXT i
FOR i = 1 TO 0
    t = t + 999
NEXT i
PRINT "T="; t
PRINT "DONE"
