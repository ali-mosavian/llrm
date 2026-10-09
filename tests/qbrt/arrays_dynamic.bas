' $DYNAMIC arrays: DIM, REDIM, ERASE on numeric (far heap) and string
' (local heap) arrays, with bounds, lower bounds and two dimensions.
REM $DYNAMIC
DIM n%(10)
DIM m&(2 TO 5, 1 TO 3)
DIM s$(3)
FOR i% = 0 TO 10: n%(i%) = i% * i%: NEXT
FOR i% = 2 TO 5
    FOR j% = 1 TO 3: m&(i%, j%) = i% * 100& + j%: NEXT
NEXT
FOR i% = 0 TO 3: s$(i%) = "item" + STR$(i%): NEXT
PRINT n%(10); m&(5, 3); s$(2)
PRINT LBOUND(m&, 1); UBOUND(m&, 1); LBOUND(m&, 2); UBOUND(m&, 2)
REDIM n%(4)
PRINT n%(0); UBOUND(n%)
ERASE m&
REDIM m&(1 TO 2, 1 TO 2)
m&(2, 2) = 7
PRINT m&(2, 2)
ERASE s$
REDIM s$(1)
s$(1) = "again"
PRINT s$(1); LEN(s$(0))
