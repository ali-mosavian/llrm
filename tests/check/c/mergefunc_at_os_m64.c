// RUN: llrm-cc %s -m64 {-Os | -O2} -S -o /dev/stdout
// Identical exported functions are one body at -Os (DEVIATE MERGEFUNC-OS, gcc -fipa-icf): the second is a `jmp` to the first.
// sfm16 float routines, shift_int32_variable and long_compare_chain took 10..72 B each at m64 -Os.
// CHECK-LABEL: twin_b proc
// CHECK[-Os]: jmp twin_a
// CHECK-NOT[-O2]: jmp twin_a
long twin_a(long x) { return x * 37 + 11; }
long twin_b(long x) { return x * 37 + 11; }
