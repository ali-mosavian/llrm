TYPE Inner
v(3) AS INTEGER
w AS INTEGER
END TYPE
TYPE Rec
a(5) AS INTEGER
b(1 TO 3, 1 TO 4) AS LONG
s(2) AS STRING * 4
d(2) AS DOUBLE
n AS INTEGER
t AS STRING * 6
END TYPE
TYPE Outer
i(2) AS Inner
q AS Inner
END TYPE
SUB t
DIM r AS Rec, rr(3) AS Rec, o AS Outer, oo(2) AS Outer
DIM x%, i%, j%, s$, d#
s$ = r.s(1)
END SUB
