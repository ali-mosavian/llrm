' RUN: llrm-qb %s --dialect {qb45 | pds71 | vbdos} --runtime {qb45 | pds71 | vbdos} -O2 -march=i486 -S -o /dev/stdout
' A loop storing one word that no one byte repeats stays a counted loop, as in the pin.
' CHECK-LABEL: FILLW proc
' CHECK: mov word ptr A%[bx], 4660
' CHECK: jb
' CHECK-NOT: rep stosw
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
