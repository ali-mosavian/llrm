' COMMON that names an array a DIM declared: the array stays static, only the scalar is common.
DIM v(1 TO 50)
COMMON v(), n
n = 3
v(n) = 2
PRINT v(3); n
