' A TYPE field's STRING * n may name a CONST declared above it
DEFINT A-Z
CONST n = 6, m = 3
TYPE T
  b AS STRING * n
  v AS STRING * m
END TYPE
DIM x AS T
x.b = "hello world"
x.v = "abcdef"
PRINT "["; x.b; "]["; x.v; "]"; LEN(x)
