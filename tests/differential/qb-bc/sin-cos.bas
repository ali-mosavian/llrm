' SIN and COS of 40 arguments, DOUBLE bytes. llrm computes them with the x87 (fsin, fcos); BC calls
' B$SIN8 and B$COS8, whose results differ in the last bits: docs/frontends/qb/divergences.md.
DEFDBL A-Z
DIM bad AS INTEGER, tot AS INTEGER
a = 0.1
FOR i% = 1 TO 40
    PRINT HEX$(CVL(LEFT$(MKD$(SIN(a)), 4))); HEX$(CVL(RIGHT$(MKD$(SIN(a)), 4))); " "; HEX$(CVL(LEFT$(MKD$(COS(a)), 4))); HEX$(CVL(RIGHT$(MKD$(COS(a)), 4)))
    a = a * 1.37 + .05
NEXT
