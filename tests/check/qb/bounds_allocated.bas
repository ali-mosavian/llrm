' RUN: llrm-qb %s -O2 -march=i486 --whole-program -fno-inline-functions -fno-inline-functions-called-once -fsanitize=bounds -S -o /dev/stdout
' Every caller DIMs what it passes, so under -fsanitize=bounds UBOUND of the
' parameter needs no allocated test and no B$UBND; after an ERASE it keeps both.
' CHECK-LABEL: ALLOC proc
' CHECK-NOT: B$UBND
' CHECK-NOT: cmp word ptr [{{.*}}+2], 0
' CHECK: ALLOC endp
' CHECK-LABEL: ERASED proc
' CHECK: B$UBND
DEFINT A-Z
DECLARE FUNCTION Alloc (q())
DECLARE FUNCTION Erased (q())
REDIM a(5)
REDIM b(7)
REDIM c(3)
ERASE c
PRINT Alloc(a()); Alloc(b()); Erased(c())

FUNCTION Alloc (q())
    Alloc = UBOUND(q)
END FUNCTION

FUNCTION Erased (q())
    Erased = UBOUND(q)
END FUNCTION
