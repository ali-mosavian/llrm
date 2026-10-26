' SCREEN with a colour switch and pages: pages are drawn to and shown; a page the mode lacks is error 5.
DEFINT A-Z
ON ERROR GOTO h
SCREEN 8, 1
PSET (10, 10), 4
PRINT POINT(10, 10)
SCREEN 7, , 1, 0
PSET (20, 20), 5
PRINT POINT(20, 20)
SCREEN , , 0, 0
PRINT POINT(20, 20)
SCREEN , , 1
PRINT POINT(20, 20)
SCREEN , , 0, 1
PRINT POINT(20, 20)
SCREEN 7, , 8
SCREEN 13, , 1
SCREEN 1, 1
PSET (5, 5), 2
PRINT POINT(5, 5)
SCREEN , , 0, 0
SCREEN 0
PRINT "done"
END
h:
PRINT "error"; ERR
RESUME NEXT
