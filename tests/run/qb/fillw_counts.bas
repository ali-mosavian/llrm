' flags: -O2 -fno-inline-functions
' A word fill counted from a symbolic bound (FOR i = 0 TO n - 1) is one rep stosw: it fills n cells and no others.
DEFINT A-Z
DECLARE SUB FillW (n)
DIM SHARED a(199)
FOR t = 1 TO 7
  READ n
  FOR i = 0 TO 199: a(i) = 0: NEXT
  FillW n
  c = 0
  FOR i = 0 TO 199
    IF a(i) <> 0 THEN c = c + 1
  NEXT
  PRINT n; c
NEXT
DATA 0,1,2,199,200,-1,-32767

SUB FillW (n)
  FOR i = 0 TO n - 1
    a(i) = 4660
  NEXT
END SUB
