' RUN: llrm-qb %s -O2 -march=i486 -fsanitize=bounds -S -o /dev/stdout
' Under -fsanitize=bounds, B$UBND stays where it can raise error 9, on the
' cold path: a parameter array, and an array after ERASE.
' CHECK-LABEL: PARAM proc
' CHECK: retf
' CHECK: B$UBND
' CHECK-LABEL: ERASED proc
' CHECK: B$ERAS
' CHECK-NOT: byte ptr
' CHECK: retf
' CHECK: B$UBND
DEFINT A-Z
DECLARE FUNCTION Param (q())
DECLARE FUNCTION Erased ()
REDIM p(4)
PRINT Param(p()); Erased

FUNCTION Param (q())
    Param = UBOUND(q)
END FUNCTION

FUNCTION Erased
    REDIM e(5)
    ERASE e
    Erased = UBOUND(e)
END FUNCTION
