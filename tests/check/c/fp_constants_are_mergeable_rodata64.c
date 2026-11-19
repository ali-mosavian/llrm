// RUN: llrm-cc %s -m64 -fno-pic -O2 -S -o /dev/stdout
// A float constant sat in the writable `.data` after the globals (floats -O2: .data 0x28, no .rodata; gcc and clang 40..64 B in .rodata.cst8). The pin keeps a 4, 8, 16 or 32-byte constant in the read-only mergeable section of its size.
// CHECK: .rodata.cst8 segment
// CHECK: $K{{[0-9]+}} label byte
// CHECK: .rodata.cst4 segment
// CHECK: $K{{[0-9]+}} label byte
double f(double x) { return x * 3.14159 + 2.71828; }
float g(float x) { return x * 1.5f + 0.25f; }
