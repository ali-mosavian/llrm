' dialect: pds71
' flags: -O2 -march=i486 --huge-arrays
' 21843 triples of INTEGERs (131058 bytes) REDIM'd /AH: 6-byte elements, a stride that does not divide 64K, over three windows. PDS /AH (BC too) refuses 21845 of them.
DEFINT A-Z
TYPE Tri
    a AS INTEGER
    b AS INTEGER
    c AS INTEGER
END TYPE
DECLARE FUNCTION BenchTri& ()
PRINT LTRIM$(STR$(BenchTri&))
END

FUNCTION BenchTri&
    DIM t AS LONG
    REDIM p(0 TO 21842) AS Tri
    FOR i = 0 TO 21842
        p(i).a = i
        p(i).c = i AND 255
    NEXT i
    t = 0
    FOR i = 0 TO 21842
        t = t + (p(i).c - p(i).a)
    NEXT i
    BenchTri& = t
END FUNCTION
