' A non-unit stride, and a second induction variable hiding inside a
' divide: i runs 0, 5, 10 ... and i \ 5 runs 0, 1, 2 ... so the division
' is a counter, not arithmetic. The exit test is on i where it could be
' on either.
DEFINT A-Z
DIM b(200)
DIM i, t
t = 0
FOR i = 0 TO 100 STEP 5
    b(i) = i \ 5
    t = t + b(i)
NEXT i
PRINT "T="; t
PRINT "DONE"
