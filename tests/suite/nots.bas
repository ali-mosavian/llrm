' NOT, EQV and IMP all go through the not pair, which BC does as two 16-bit
' nots. An idiom the lifter does not know invalidates the register tracking
' and costs everything after it until the next load, so it is worth more than
' its own three bytes.
DEFINT A-Z
DIM a AS LONG, b AS LONG, r AS LONG

a = 305419896
b = 252645135

r = NOT a
PRINT "NOT="; r
r = a EQV b
PRINT "EQV="; r
r = a IMP b
PRINT "IMP="; r
r = NOT (a AND b)
PRINT "NAND="; r
r = (NOT a) OR b
PRINT "NOTOR="; r
PRINT "DONE"
