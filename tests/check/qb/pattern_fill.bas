' RUN: llrm-qb %s --dialect {qb45 | pds71 | vbdos} --runtime {qb45 | pds71 | vbdos} -O2 -march=i486 -S -o /dev/stdout
' A loop storing one word that no one byte repeats is `rep stosw` where the fill pays (loopidioms). The pin
' keeps a counted loop: DEVIATE, the nib twin's 100 words execute 12 instructions as the fill, 609 as the loop.
' CHECK-LABEL: FILLW proc
' CHECK: mov ax, 4660
' CHECK: rep stosw
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
