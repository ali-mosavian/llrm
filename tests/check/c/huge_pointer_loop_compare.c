// RUN: llrm-cc %s -m16 -mabi=ia16 -march=i386 -Os --isel=dag -S -o /dev/stdout
// A loop on `p < e` of two __huge pointers panicked under --isel=dag ("invalid integer condition SetOGe"): the not-setcc fold took the far pointer type for a float and inverted `ult` as a float compare.
long __huge big[20000];
long walk(long __huge *p, long __huge *e) { long t = 0; while (p < e) { t += *p; p++; } return t; }
// CHECK-LABEL: _walk proc
// A split copy of a stack-argument load is made again where it is read (`scanRemattable` follows the original), not stored: a `[bp-N]` spill sat between the add and the setb. The compare of the two 32-bit pointers is the borrow of the offsets into the selectors (`cmp; sbb`), and the step's carry moves the selector: not `setae` and a select, nor a dword add and a shift.
// CHECK: sbb {{[a-z][a-z]}}, {{[a-z][a-z]}}
// CHECK: add eax, dword ptr es:
// CHECK: add {{[a-z][a-z]}}, 4
// CHECK-NOT: word ptr [bp-
// CHECK: setb
// CHECK: sbb {{[a-z][a-z]}}, {{[a-z][a-z]}}
// CHECK-NOT: setae
