' CSRLIN and POS(0) are the cursor's 1-based row and column; the runtime had neither entry, so a
' program using them did not link.
CLS
PRINT "ab";
PRINT POS(0); CSRLIN
LOCATE 7, 12
PRINT POS(0); CSRLIN
PRINT "x";
PRINT POS(0)
