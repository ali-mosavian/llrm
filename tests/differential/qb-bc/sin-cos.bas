' bc: /O /E /X
' flags: -fexcess-precision=standard
' SIN and COS of 40 arguments to 11 decimals. llrm computes them with the x87 (fsin, fcos), BC calls B$SIN8
' and B$COS8; their results differ in the last bits (docs/frontends/qb/divergences.md), and so does the fsin of one
' dosbox-x build from the next (the bytes of the DOUBLEs changed by up to 21 ulps between builds). 11 decimals are
' compared instead: every value is 900 ulps from a rounding boundary, and a wrong sin changes them. Recorded on
' dosbox-x 2026.06.02; checked against exact arithmetic, all 80 values agree.
DEFDBL A-Z
DIM bad AS INTEGER, tot AS INTEGER
a = 0.1
FOR i% = 1 TO 40
    PRINT USING "+#.########### +#.###########"; SIN(a); COS(a)
    a = a * 1.37 + .05
NEXT
