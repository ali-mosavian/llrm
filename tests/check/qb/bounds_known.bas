' RUN: llrm-qb %s -O2 --cpu 486 -S -o /dev/stdout
' A bound a dominating DIM or REDIM states is that value: a constant, or the
' variable it was handed, also across a call. LBOUND and UBOUND read the
' descriptor and called B$LBND/B$UBND where it might be unallocated. No load reads the
' frame: the entry's stores zero the descriptor, as own frames do.
' CHECK-LABEL: FIXED proc
' CHECK-NOT: BND
' CHECK-NOT: {{, [a-z]*word ptr \[bp}}
' CHECK: mov ax, 9912
' CHECK-LABEL: STATED proc
' CHECK: B$RDIM
' CHECK-NOT: BND
' CHECK-NOT: ptr [bp-{{(5\d|6[0-4])}}]
' CHECK: mov ax, 5
' CHECK: STATED endp
DEFINT A-Z
DECLARE FUNCTION Fixed ()
DECLARE FUNCTION Stated (n, m)
PRINT Fixed; Stated(3, 4)

FUNCTION Fixed
    DIM a(10 TO 99, 2)
    Fixed = UBOUND(a) * 100 + LBOUND(a) + UBOUND(a, 2) - LBOUND(a, 2)
END FUNCTION

FUNCTION Stated (n, m)
    x = n: y = m
    REDIM b(x, 5 TO y)
    PRINT x
    Stated = UBOUND(b, 1) - x + UBOUND(b, 2) - y + LBOUND(b, 2) - LBOUND(b, 1)
END FUNCTION
