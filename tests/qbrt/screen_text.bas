' COLOR, LOCATE, CLS, VIEW PRINT and PRINT on the screen: colours, positions,
' wrapping at the right edge, and scrolling inside a PRINT window.
CLS
COLOR 14, 1
PRINT "yellow on blue"
COLOR 7, 0
LOCATE 5, 10: PRINT "at 5,10";
LOCATE 6, 70: PRINT "wraps past the edge of the line"
COLOR 12: PRINT "red"; : COLOR , 2: PRINT " red on green"
COLOR 31, 0: PRINT "blinking white"
COLOR 7, 0
FOR i% = 1 TO 4: PRINT i%, i% * i%, "x"; CHR$(9); "tab": NEXT
VIEW PRINT 10 TO 14
FOR i% = 1 TO 9: PRINT "line"; i%: NEXT
VIEW PRINT
LOCATE 20, 1: PRINT "bye"
LOCATE 22, 5: COLOR 2, 4: PRINT "end";
' The colours the cells ended with, read back from video memory.
DIM attr%(24, 3)
DEF SEG = &HB800
FOR r% = 0 TO 24
    attr%(r%, 0) = PEEK(r% * 160 + 1)
    attr%(r%, 1) = PEEK(r% * 160 + 11)
    attr%(r%, 2) = PEEK(r% * 160 + 41)
    attr%(r%, 3) = PEEK(r% * 160 + 159)
NEXT
DEF SEG
COLOR 7, 0
CLS
FOR r% = 0 TO 24
    PRINT r%; attr%(r%, 0); attr%(r%, 1); attr%(r%, 2); attr%(r%, 3)
NEXT
LOCATE 25, 1
