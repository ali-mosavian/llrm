' Text on the SCREEN 13 graphics screen: more lines than rows, so it scrolls, with a VIEW PRINT window, LOCATE and a cleared screen.
SCREEN 13
FOR i% = 1 TO 40
  PRINT "line"; i%; "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMN"
NEXT
LOCATE 3, 5: PRINT "here";
LOCATE 10, 12: PRINT "a tab"; TAB(30); "x"
VIEW PRINT 5 TO 9
FOR i% = 1 TO 8: PRINT "w"; i%: NEXT
VIEW PRINT
LINE (0, 0)-(30, 30), 1, BF
CLS
PRINT "after cls"
LINE (20, 20)-(60, 40), 1, BF
LOCATE 12, 1
FOR i% = 1 TO 14: PRINT i%: NEXT
SLEEP
