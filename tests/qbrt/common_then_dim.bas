' COMMON names an array without bounds and a later DIM gives them: the array is dynamic.
DEFINT A-Z
TYPE T
  a AS INTEGER
  b AS LONG
END TYPE
COMMON SHARED /ui/ g() AS T
COMMON v(), n
DIM g(10) AS T
DIM v(1 TO 5)
g(2).a = 4: g(10).b = 70000
n = 3
v(n) = 2
PRINT g(2).a; g(10).b; v(3); n; UBOUND(g); LBOUND(v)
