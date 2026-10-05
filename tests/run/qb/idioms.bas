' Fill and copy loops as programs write them: clearing and scrolling a buffer, filling a
' palette, copying rows and records, and the loops that look alike but are not one move:
' a smear, a changed value, a second store, a stride that is not the cell, a source the body
' writes. Each prints a checksum that weighs every cell by its place; idioms.c is the same.
DEFINT A-Z
DECLARE SUB Init ()
DECLARE SUB ClearRows (rows)
DECLARE SUB SetPal (n, w)
DECLARE SUB DoCopy (n)
DECLARE SUB ScrollUp (n)
DECLARE SUB ScrollDown (n)
DECLARE SUB Smear (n)
DECLARE SUB Changed (n)
DECLARE SUB TwoStores (n)
DECLARE SUB Strides (n)
DECLARE SUB Written (n)
DECLARE SUB Blk (w)
DECLARE SUB CopyLong (n)
DECLARE SUB FillLong (n, v AS LONG)
DECLARE FUNCTION Lim (n, cap)
DECLARE FUNCTION SumA& ()
DECLARE FUNCTION SumB& ()
DECLARE FUNCTION SumC& ()
DECLARE FUNCTION SumPal& ()
DECLARE FUNCTION SumL& ()
DIM SHARED a(159), b(159), c(159), pal(63)
DIM SHARED l(39) AS LONG, m(39) AS LONG
DATA 0, 1, 2, 17, 64, 100
FOR t = 0 TO 5
  READ n
  Init
  ClearRows n \ 10
  PRINT LTRIM$(STR$(SumA&))
  FOR i = 0 TO 63: pal(i) = -1: NEXT
  k = Lim(n, 64)
  SetPal k, ((n AND 255) * 256) OR &H34
  PRINT LTRIM$(STR$(SumPal&))
  Init
  DoCopy n
  PRINT LTRIM$(STR$(SumA&))
  Init
  ScrollUp n
  PRINT LTRIM$(STR$(SumA&))
  Init
  ScrollDown n
  PRINT LTRIM$(STR$(SumA&))
  Init
  Smear n
  PRINT LTRIM$(STR$(SumA&))
  Init
  Changed n
  PRINT LTRIM$(STR$(SumA&))
  Init
  TwoStores n
  PRINT LTRIM$(STR$(SumA& + SumC&))
  Init
  Strides n
  PRINT LTRIM$(STR$(SumA&))
  Init
  Written n
  PRINT LTRIM$(STR$(SumA& + SumB&))
  Init
  Blk n \ 10
  PRINT LTRIM$(STR$(SumA&))
  FOR i = 0 TO 39: l(i) = &H1000 + i: m(i) = &H2000 + 3 * i: NEXT
  k = Lim(n, 40)
  CopyLong k
  PRINT LTRIM$(STR$(SumL&))
  FOR i = 0 TO 39: l(i) = -1: NEXT
  k = Lim(n, 40)
  FillLong k, &H123456 + n
  PRINT LTRIM$(STR$(SumL&))
NEXT
END

FUNCTION Lim (n, cap)
  IF n < cap THEN Lim = n ELSE Lim = cap
END FUNCTION

SUB Init
  FOR i = 0 TO 159
    a(i) = i + 1000: b(i) = i * 3 + 1: c(i) = i * 5 + 2
  NEXT
END SUB

FUNCTION SumA&
  s& = 0
  FOR i = 0 TO 159: s& = s& + CLNG(i + 1) * a(i): NEXT
  SumA& = s&
END FUNCTION

FUNCTION SumB&
  s& = 0
  FOR i = 0 TO 159: s& = s& + CLNG(i + 1) * b(i): NEXT
  SumB& = s&
END FUNCTION

FUNCTION SumC&
  s& = 0
  FOR i = 0 TO 159: s& = s& + CLNG(i + 1) * c(i): NEXT
  SumC& = s&
END FUNCTION

FUNCTION SumPal&
  s& = 0
  FOR i = 0 TO 63: s& = s& + CLNG(i + 1) * pal(i): NEXT
  SumPal& = s&
END FUNCTION

FUNCTION SumL&
  s& = 0
  FOR i = 0 TO 39: s& = s& + CLNG(i + 1) * l(i): NEXT
  SumL& = s&
END FUNCTION

SUB ClearRows (rows)
  FOR y = 0 TO rows - 1
    FOR x = 0 TO 15
      a(y * 16 + x) = 4660
    NEXT
  NEXT
END SUB

SUB SetPal (n, w)
  FOR i = 0 TO n - 1
    pal(i) = w
  NEXT
END SUB

SUB DoCopy (n)
  FOR i = 0 TO n - 1
    a(i) = b(i)
  NEXT
END SUB

SUB ScrollUp (n)
  FOR i = 0 TO n - 1
    a(i) = a(i + 16)
  NEXT
END SUB

SUB ScrollDown (n)
  FOR i = n - 1 TO 0 STEP -1
    a(i + 16) = a(i)
  NEXT
END SUB

SUB Smear (n)
  FOR i = 0 TO n - 1
    a(i + 1) = a(i)
  NEXT
END SUB

SUB Changed (n)
  FOR i = 0 TO n - 1
    a(i) = b(i) + 1
  NEXT
END SUB

SUB TwoStores (n)
  FOR i = 0 TO n - 1
    a(i) = b(i)
    c(i) = 0
  NEXT
END SUB

SUB Strides (n)
  FOR i = 0 TO n \ 2 - 1
    a(2 * i) = b(i)
  NEXT
END SUB

SUB Written (n)
  FOR i = 0 TO n - 1
    b(i) = 0
    a(i) = b(i)
  NEXT
END SUB

SUB Blk (w)
  FOR y = 0 TO 3
    FOR x = 0 TO w - 1
      a(y * 16 + x) = b(y * 20 + x)
    NEXT
  NEXT
END SUB

SUB CopyLong (n)
  FOR i = 0 TO n - 1
    l(i) = m(i)
  NEXT
END SUB

SUB FillLong (n, v AS LONG)
  FOR i = 0 TO n - 1
    l(i) = v
  NEXT
END SUB
