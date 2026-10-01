' ON n GOSUB with n outside 0..255 raises ERROR 5 in the ON itself: RESUME runs the
' ON again, with the handler's x, and prints err 5 / a / after.
DEFINT A-Z
ON ERROR GOTO handler
c = 0
x = 300
ON x GOSUB a, b
PRINT "after"
END
a: PRINT "a": RETURN
b: PRINT "b": RETURN
handler: PRINT "err"; ERR: c = c + 1: IF c > 3 THEN PRINT "loop": END
x = 1: RESUME
