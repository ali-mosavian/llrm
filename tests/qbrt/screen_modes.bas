' SCREEN 0, WIDTH and VIEW PRINT leave the text where it is.
CLS
FOR i% = 1 TO 10: PRINT "row"; i%: NEXT
SCREEN 0
WIDTH 80, 25
PRINT "after SCREEN 0 and WIDTH"
VIEW PRINT 3 TO 6
CLS
PRINT "in the window"
FOR i% = 1 TO 6: PRINT "w"; i%: NEXT
VIEW PRINT 8 TO 10
PRINT "second window"
LOCATE 9, 1: PRINT "nine"
VIEW PRINT
PRINT "outside"
LOCATE 25, 1: PRINT "bottom";
LOCATE 25, 1
