' A word or dword stored in n cells by a loop: the cells filled and no others,
' for no trips, one, odd counts and the whole array.
DEFINT A-Z
DECLARE SUB FillW (n, v)
DECLARE SUB FillL (n, v AS LONG)
DIM SHARED a(63), b(63) AS LONG
FOR t = 0 TO 5
  READ n
  FOR i = 0 TO 63
    a(i) = 0
    b(i) = 0
  NEXT
  FillW n, 4660
  FillL n, 305419896
  s& = 0
  c = 0
  FOR i = 0 TO 63
    s& = s& + a(i)
    IF b(i) <> 0 THEN c = c + 1
  NEXT
  PRINT n; s&; c; b(63)
NEXT
DATA 0, 1, 2, 17, 63, 64

SUB FillW (n, v)
  FOR i = 0 TO n - 1
    a(i) = v
  NEXT
END SUB

SUB FillL (n, v AS LONG)
  FOR i = 0 TO n - 1
    b(i) = v
  NEXT
END SUB
