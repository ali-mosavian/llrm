// RUN: llrm-cc %s -m64 {-Os | -O2} -S -o /dev/stdout
// A constant memcpy is a `rep movs` at -Os, not the library call (DEVIATE REP-MOVS-OS): memory_expansion -398 B at m64 -Os.
// The widest whole block (#9356): 200 B is `rep movsd` of 50, not `rep movsb`.
// CHECK-LABEL: copy proc
// CHECK-NOT[-Os]: call
// CHECK[-Os]: rep movs{{[bdwq]}}
// CHECK-NOT[-O2]: rep movs
extern char dst[200], src[200];
void copy(void) { __builtin_memcpy(dst, src, 200); }
