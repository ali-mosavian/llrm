' INPUT's less usual lines: too few values, reals, empty, backspace, quoted
' strings.
CLS
INPUT "two"; a%, b%
PRINT "got"; a%; b%
INPUT x#
PRINT "got"; x#
INPUT n%
PRINT "got"; n%
INPUT m%
PRINT "got"; m%
INPUT q$
PRINT "got ["; q$; "]"
LOCATE 25, 1
