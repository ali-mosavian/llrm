// RUN: llrm-cc %s --target=x86_32-wat -mabi=sysv -march=i486 -Os -S -o /dev/stdout
// DEVIATE REP-MOVS-OS and REP-STOS-OS stop at the pin's 128 bytes: 4000 2-aligned bytes were `rep movsw` (scroll -Os
// 682,269 clk against the call's 535,965), a 512-byte fill `rep stosw` (histo); the calls go by dwords. Within the limit
// both stay `rep`.
// CHECK-LABEL: _big proc
// CHECK-NOT: rep movs
// CHECK: call _memcpy
// CHECK: _big endp
// CHECK-LABEL: _small proc
// CHECK-NOT: call
// CHECK: rep movs
// CHECK-LABEL: _fill proc
// CHECK-NOT: rep stos
// CHECK: call _memset
// CHECK: _fill endp
// CHECK-LABEL: _fill_small proc
// CHECK-NOT: call
// CHECK: rep stos
extern short dst[2000], src[2000];
void big(void) { __builtin_memcpy(dst, src, 4000); }
void small(void) { __builtin_memcpy(dst, src, 120); }
void fill(void) { __builtin_memset(dst, 0, 512); }
void fill_small(void) { __builtin_memset(dst, 0, 120); }
