' LBOUND and UBOUND of static and dynamic arrays, by constant and by variable dimension: on dos32 they read
' QB's descriptor offsets and so the wrong fields.
DIM a%(2 TO 5, -1 TO 3)
REDIM b&(0 TO 9)
d% = 2
PRINT LBOUND(a%, 1); UBOUND(a%, 1); LBOUND(a%, 2); UBOUND(a%, 2)
PRINT LBOUND(a%, d%); UBOUND(a%, d%); LBOUND(b&); UBOUND(b&)
REDIM c&(5 TO 7, 1 TO 2)
PRINT LBOUND(c&, 1); UBOUND(c&, 1); LBOUND(c&, d%); UBOUND(c&, d%)
a%(3, 0) = 9
c&(6, 2) = 70000
PRINT a%(3, 0); c&(6, 2)
