// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -Os -S -o /dev/stdout
// At -Os a byte remainder by a constant stays a `div` (isIntDivCheap holds for i8 once DIV8r selects). It was the i32 multiply, shift,
// multiply and subtract: +54 B at m16 and +55 B at m32 on tests/run/c/div8_os.c.
unsigned char r(unsigned char a) { return a % 6; }
// CHECK-LABEL: _r proc
// CHECK: div cl
// CHECK-NOT: imul
// CHECK: _r endp
