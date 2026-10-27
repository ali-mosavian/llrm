// RUN: llrm-c %s -m16 -mabi=ia16 -march=i386 -Os --isel=dag -S -o /dev/stdout
// A loop on `p < e` of two __huge pointers panicked under --isel=dag ("invalid integer condition SetOGe"): the not-setcc fold took the far pointer type for a float and inverted `ult` as a float compare.
long __huge big[20000];
long walk(long __huge *p, long __huge *e) { long t = 0; while (p < e) { t += *p; p++; } return t; }
// CHECK-LABEL: _walk proc
// CHECK: add eax, dword ptr es:
