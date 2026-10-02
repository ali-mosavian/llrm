' A chain the widening ends without a store. PRINT consumes the pair, so the
' restore has to hand it back, and the last pair is two two-byte
' instructions -- `neg ax / neg dx`, four bytes. The restore used to be
' placed four bytes back from the end of the chain to make its own span the
' four it emits, which is exactly the address the widened negate already
' held: two operations on one address, and layout.py keys them by address.
DEFINT A-Z
DIM a AS LONG, b AS LONG

a = 305419896
b = 252645135

PRINT "A="; -(NOT a)
PRINT "B="; -(NOT (a AND b))
PRINT "C="; NOT (a OR b)
PRINT "D="; -(a AND b)
PRINT "DONE"
