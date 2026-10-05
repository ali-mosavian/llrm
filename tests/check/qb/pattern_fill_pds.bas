' RUN: llrm-qb %s --dialect pds71 --runtime pds71 -O2 --cpu 486 -S -o /dev/stdout
' A loop storing one word that no one byte repeats is one rep stosw: the
' loop cost a store and a branch a trip.
' CHECK-LABEL: FILLW proc
' CHECK: rep stosw
' CHECK-NOT: jne
' CHECK: FILLW endp
DEFINT A-Z
DECLARE SUB FillW (n)
DIM SHARED a(199)
FillW 200
PRINT a(7)

SUB FillW (n)
  FOR i = 0 TO n - 1
    a(i) = 4660
  NEXT
END SUB
