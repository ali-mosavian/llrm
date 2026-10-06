// RUN: llrm-c %s -O2 -march=i486 -S -o /dev/stdout
// A loop storing one word that no one byte repeats is one rep stosw.
// CHECK-LABEL: _fillw proc
// CHECK: rep stosw
// CHECK-NOT: jl
// CHECK: _fillw endp
short a[100];
void fillw(void) { int i; for (i = 0; i < 100; i++) a[i] = 4660; }
