' bc: /O /E /X
' flags: -fsanitize=bounds
' LBOUND and UBOUND wherever the frontend knows the bounds a DIM or REDIM
' handed the runtime, and wherever another statement may have changed them.
DEFINT A-Z
DECLARE SUB Param (q())
DECLARE SUB Grow (q())
DECLARE SUB Shared2 ()
DECLARE SUB Raises ()
DECLARE SUB Locals (n, m)
DECLARE SUB Handed ()
DIM SHARED s(3 TO 4, -2 TO 8)
REDIM SHARED g(1 TO 2)
ON ERROR GOTO handler

Locals 6, 9

Handed

' The module's handler REDIMs g and RESUMEs here and in Raises.
REDIM g(5)
ERROR 5
PRINT "resumed"; UBOUND(g)
Raises

' ERASE, then each bound raises error 9.
REDIM e(4)
ERASE e
u = -1: u = UBOUND(e): PRINT "erased"; u
u = -1: u = LBOUND(e): PRINT "erased"; u
REDIM e(-3 TO 3)
PRINT "again"; LBOUND(e); UBOUND(e)
u = -1: u = UBOUND(s, 3): PRINT "rank"; u
END

handler:
PRINT "error"; ERR
IF ERR = 5 THEN REDIM g(1 TO 12)
IF ERR = 6 THEN REDIM g(2 TO 13)
RESUME NEXT

' A procedure's REDIM of the array it is handed, and of a shared one, in a
' procedure without ON ERROR, where nothing else drops what it holds.
SUB Handed
    REDIM p(5 TO 7, 2)
    Param p()
    Grow p()
    PRINT "grown"; LBOUND(p, 1); UBOUND(p, 1); LBOUND(p, 2); UBOUND(p, 2)
    REDIM g(1 TO 3)
    Shared2
    PRINT "shared"; UBOUND(g)
END SUB

SUB Grow (q())
    REDIM q(-1 TO 1, 4 TO 6)
END SUB

SUB Locals (n, m)
    DIM a(10 TO 99)
    REDIM b(n, 5 TO m)
    t& = 0
    FOR i = LBOUND(a) TO UBOUND(a)
        t& = t& + i
    NEXT
    PRINT "local"; t&; UBOUND(b, 2) - LBOUND(b, 1); UBOUND(b, 1); LBOUND(b, 2)
    PRINT "after print"; UBOUND(b); UBOUND(b, 2)
    ' Bounds that change in a loop, and a REDIM only some paths run.
    FOR k = 1 TO 3
        REDIM c(k TO 2 * k, 1 TO k)
        PRINT "loop"; LBOUND(c); UBOUND(c); UBOUND(c, 2)
        IF k = 2 THEN REDIM c(-k TO k, 0 TO 1)
        PRINT "maybe"; LBOUND(c); UBOUND(c); LBOUND(c, 2); UBOUND(c, 2)
    NEXT
    ' A dimension not known until run time.
    FOR d = 1 TO 2
        PRINT "dim"; d; LBOUND(b, d); UBOUND(b, d)
    NEXT
END SUB

SUB Param (q())
    PRINT "param"; LBOUND(q); UBOUND(q); LBOUND(q, 2); UBOUND(q, 2)
    FOR d = 1 TO 2
        PRINT "pdim"; LBOUND(q, d); UBOUND(q, d)
    NEXT
END SUB

SUB Raises
    PRINT "before"; UBOUND(g)
    ERROR 6
    PRINT "after"; LBOUND(g); UBOUND(g)
END SUB

SUB Shared2
    PRINT "s"; LBOUND(s, 1); UBOUND(s, 1); LBOUND(s, 2); UBOUND(s, 2)
    REDIM g(7 TO 9)
END SUB
