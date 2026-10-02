' Long comparison, which BC compiles to a call into B$CPI4 and qbopt
' replaces with one 32-bit cmp. Comparison pushes its left operand first
' where multiply and divide push it second, so an operand order that is
' backwards is a different answer rather than a crash -- and every pair
' here is asymmetric, so it shows.
DEFINT A-Z
DIM a1 AS LONG, a2 AS LONG
DIM b1 AS LONG, b2 AS LONG
DIM c1 AS LONG, c2 AS LONG
DIM d1 AS LONG, d2 AS LONG
a1 = -1   ' unsigned on the high half says the opposite of signed on the whole
a2 = 1
b1 = -2147483647 - 1   ' the two extremes
b2 = 2147483647
c1 = 65535   ' either side of the halfway boundary
c2 = 65536
d1 = &H12340000   ' identical high halves, so only the low one decides
d2 = &H1234FFFF

PRINT "ALT="; (a1 < a2); (a2 < a1)
PRINT "ALE="; (a1 <= a2); (a2 <= a1)
PRINT "AGT="; (a1 > a2); (a2 > a1)
PRINT "AGE="; (a1 >= a2); (a2 >= a1)
PRINT "AEQ="; (a1 = a2); (a2 = a1)
PRINT "ANE="; (a1 <> a2); (a2 <> a1)
PRINT "BLT="; (b1 < b2); (b2 < b1)
PRINT "BLE="; (b1 <= b2); (b2 <= b1)
PRINT "BGT="; (b1 > b2); (b2 > b1)
PRINT "BGE="; (b1 >= b2); (b2 >= b1)
PRINT "BEQ="; (b1 = b2); (b2 = b1)
PRINT "BNE="; (b1 <> b2); (b2 <> b1)
PRINT "CLT="; (c1 < c2); (c2 < c1)
PRINT "CLE="; (c1 <= c2); (c2 <= c1)
PRINT "CGT="; (c1 > c2); (c2 > c1)
PRINT "CGE="; (c1 >= c2); (c2 >= c1)
PRINT "CEQ="; (c1 = c2); (c2 = c1)
PRINT "CNE="; (c1 <> c2); (c2 <> c1)
PRINT "DLT="; (d1 < d2); (d2 < d1)
PRINT "DLE="; (d1 <= d2); (d2 <= d1)
PRINT "DGT="; (d1 > d2); (d2 > d1)
PRINT "DGE="; (d1 >= d2); (d2 >= d1)
PRINT "DEQ="; (d1 = d2); (d2 = d1)
PRINT "DNE="; (d1 <> d2); (d2 <> d1)
PRINT "DONE"
