' WIDTH with one argument: BC passes -1 for the missing rows, which keeps the screen's height
DEFINT A-Z
ON ERROR GOTO h
WIDTH 80
PRINT "one"
WIDTH 80, 25
PRINT "done"
END
h:
PRINT "error"; ERR
RESUME NEXT
