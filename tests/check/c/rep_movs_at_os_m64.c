// RUN: llrm-cc %s -m64 {-Os | -O2} -S -o /dev/stdout
// A constant memcpy is `rep movsb` at -Os, not the library call (DEVIATE REP-MOVS-OS): memory_expansion -398 B at m64 -Os.
// CHECK-LABEL: copy proc
// CHECK[-Os]: rep movsb
// CHECK-NOT[-O2]: rep movsb
extern char dst[200], src[200];
void copy(void) { __builtin_memcpy(dst, src, 200); }
