' RUN: llrm-qb %s --dialect vbdos --runtime vbdos -O2 --cpu 486 -fno-inline-functions -S -o /dev/stdout
' Loops people write that are one string move, and look-alikes that are not.
' CHECK-LABEL: SETPAL proc
' CHECK: rep stosw
' CHECK: SETPAL endp
' CHECK-LABEL: DOCOPY proc
' CHECK: rep movsd
' CHECK-NOT: std
' CHECK: DOCOPY endp
' CHECK-LABEL: SCROLLUP proc
' CHECK: rep movsd
' CHECK: SCROLLUP endp
' CHECK-LABEL: SCROLLDOWN proc
' CHECK: std
' CHECK: rep movsd
' CHECK: cld
' CHECK: SCROLLDOWN endp
' CHECK-LABEL: SMEAR proc
' CHECK-NOT: rep
' CHECK: SMEAR endp
' CHECK-LABEL: CHANGED proc
' CHECK-NOT: rep
' CHECK: CHANGED endp
' CHECK-LABEL: TWOSTORES proc
' CHECK-NOT: rep
' CHECK: TWOSTORES endp
' CHECK-LABEL: STRIDES proc
' CHECK-NOT: rep
' CHECK: STRIDES endp
' CHECK-LABEL: WRITTEN proc
' CHECK-NOT: rep
' CHECK: WRITTEN endp
' CHECK-LABEL: BLK proc
' CHECK: rep movsd
' CHECK: BLK endp
DEFINT A-Z
DECLARE SUB DoCopy (n)
DECLARE SUB ScrollUp (n)
DECLARE SUB ScrollDown (n)
DECLARE SUB SetPal (n, w)
DECLARE SUB Blk (w)
DECLARE SUB Smear (n)
DECLARE SUB Changed (n)
DECLARE SUB TwoStores (n)
DECLARE SUB Strides (n)
DECLARE SUB Written (n)
DIM SHARED a(159), b(159), c(159), pal(63)
DIM SHARED l(39) AS LONG, m(39) AS LONG
n = INT(RND * 10)
DoCopy n: ScrollUp n: ScrollDown n: SetPal n, n: Blk n
Smear n: Changed n: TwoStores n: Strides n: Written n
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
