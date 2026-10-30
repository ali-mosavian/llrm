' flags: -O1 | -O2 | -Os
' dialect: qb45 pds71
' Premise: a$ (a string, 16 bytes) is scanned and modified byte by byte through DEF SEG = VARSEG(a$) and SADD(a$); then printed. The string is not an array; no errors.
DEFINT A-Z
DIM SHARED a AS STRING, b AS STRING, n AS LONG
a = "Hello, Far World"
b = a + "!"
p = SADD(a)
DEF SEG = VARSEG(a)
FOR i = 0 TO LEN(a) - 1
  c = PEEK(p + i)
  n = n + c
  IF c >= 65 AND c <= 90 THEN
    POKE p + i, c + 32
  ELSEIF c >= 97 AND c <= 122 THEN
    POKE p + i, c - 32
  END IF
NEXT i
DEF SEG
PRINT a; "|"; b; "|"; n; LEN(a)
