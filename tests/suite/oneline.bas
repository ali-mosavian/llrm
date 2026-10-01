' One-line IFs: an ELSE pairs with the nearest unmatched IF, and a line number
' after THEN or ELSE is a GOTO.
DEFINT A-Z
FOR a = 0 TO 1
    FOR b = 0 TO 1
        IF a THEN IF b THEN x = 1 ELSE x = 2 ELSE x = 3
        PRINT "N"; a; b; x
    NEXT b
NEXT a
FOR a = 0 TO 1
    IF a THEN 100 ELSE 200
100 PRINT "THEN"; a: GOTO 300
200 PRINT "ELSE"; a
300 NEXT a
