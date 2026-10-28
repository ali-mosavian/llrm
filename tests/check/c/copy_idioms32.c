// RUN: llrm-cc %s -m32 {-O2 | -Os} -march=i486 -fno-inline-functions -S -o /dev/stdout
// bench/scroll on -m32: `short` counters over 32-bit pointers were word-at-a-time loops
// (1196306 instructions, gcc 176970); each is one memmove. The pin's idiom pass makes the
// llvm.memmove, and Watcom -m32 has memmove_ to call: no inline string move.
// CHECK-LABEL: up_ proc
// CHECK: call memmove_
// CHECK: up_ endp
// CHECK-LABEL: down_ proc
// CHECK: call memmove_
// CHECK: down_ endp
// CHECK-LABEL: smear_ proc
// CHECK-NOT: rep
// CHECK-NOT: memmove_
// CHECK: smear_ endp
short s[2000], t[2000];
void up(void) { short i; for (i = 0; i < 1920; ++i) s[i] = s[i + 80]; }
void down(void) { short i; for (i = 1919; i >= 0; --i) s[i + 80] = s[i]; }
void smear(void) { short i; for (i = 0; i < 1920; ++i) s[i + 80] = s[i]; }
