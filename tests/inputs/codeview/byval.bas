' A BYVAL LONG parameter beside a BYREF one -- does BYVAL skip the two-hop
' pointer-wrapper chain and carry the primitive type_index directly?
DEFINT A-Z
DECLARE FUNCTION AddRef& (n AS LONG)
DECLARE FUNCTION AddVal& (BYVAL n AS LONG)

DIM a AS LONG, b AS LONG
a = 100
b = 200

PRINT AddRef(a)
PRINT AddVal(b)
PRINT "DONE"

FUNCTION AddRef& (n AS LONG)
    AddRef& = n + 1
END FUNCTION

FUNCTION AddVal& (BYVAL n AS LONG)
    AddVal& = n + 2
END FUNCTION
