// RUN: llrm-c %s -O2 --cpu 486 --target x86-code32 -S -o /dev/stdout
// An `unsigned short` counter on a 32-bit `int` was `zext` and `slt` per trip
// (code32 sieve 19277 executed, code16 12107): no memset, no count to the bound.
// CHECK-LABEL: _clear proc
// CHECK: rep stosb
// CHECK-NOT: {{jb|jl}}
// CHECK: _clear endp
// CHECK-LABEL: _total proc
// CHECK-NOT: movzx {{e[a-d]x}}, {{[a-d]x}}
// CHECK-NOT: add {{[a-d]x}}, 1
// CHECK: _total endp
unsigned char buf[100];
void clear(unsigned short n) { unsigned short i; for (i = 0; i < n; ++i) buf[i] = 0; }
unsigned long total(const unsigned char *p) { unsigned short i; unsigned long s = 0; for (i = 0; i < 100; ++i) s += (unsigned long)p[i] * i; return s; }
