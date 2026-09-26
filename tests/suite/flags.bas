' Widening changes which flags a following jcc reads. BC leaves the HIGH
' half's flags and one 32-bit operation leaves the whole result's, so where the
' high half is zero and the whole result is not, ZF goes the other way.
DEFINT A-Z
DIM a AS LONG, b AS LONG, r AS LONG

a = 65535                ' &H0000FFFF -- high half zero
b = 61680                ' &H0000F0F0
r = a AND b              ' &H0000F0F0: nonzero, but its high half is not
IF r = 0 THEN PRINT "SPLIT=zero" ELSE PRINT "SPLIT=nonzero"
IF (a AND b) = 0 THEN PRINT "FUSED=zero" ELSE PRINT "FUSED=nonzero"

a = -65536               ' &HFFFF0000 -- low half zero, high half not
b = -65536
r = a AND b
IF r = 0 THEN PRINT "MIRROR=zero" ELSE PRINT "MIRROR=nonzero"

a = 0
b = 0
r = a AND b
IF r = 0 THEN PRINT "BOTH=zero" ELSE PRINT "BOTH=nonzero"

a = 65536
b = 1
r = a - b                ' crosses the halfway boundary downward
IF r < 0 THEN PRINT "SIGN=neg" ELSE PRINT "SIGN=pos"
PRINT "DONE"
