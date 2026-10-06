' dialect: pds71
' flags: -O2 -march=i486 --huge-arrays
' /AH elements addressed inline, with bounds known only at run time: each
' lower bound is subtracted from the descriptor's, as B$HARY did. 32767
' elements is a dimension's most (#359: B$RDIM refuses 32768, in BC too).
DEFINT A-Z
DIM n AS LONG
n = 32766
lo = -2
REDIM h(n) AS LONG
REDIM g(lo TO 0, lo TO n + lo) AS INTEGER
h(0) = 5
h(32766) = 7
g(lo, lo) = 11
g(0, 32764) = 13
g(-1, 20000) = 17
PRINT h(0); h(32766); g(-2, -2); g(0, 32764); g(-1, 20000); LBOUND(g, 2)
