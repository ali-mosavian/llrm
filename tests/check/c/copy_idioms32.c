// RUN: llrm-c %s -m32 {-O2 | -Os} -march=i486 -fno-inline-functions -S -o /dev/stdout
// bench/scroll on -m32: `short` counters over 32-bit pointers were word-at-a-time loops
// (1196306 instructions, gcc 176970); each is one string move.
// CHECK-LABEL: up_ proc
// CHECK: rep movsd
// CHECK-NOT: std
// CHECK: up_ endp
// CHECK-LABEL: down_ proc
// CHECK: std
// CHECK: rep movsd
// CHECK: cld
// CHECK: down_ endp
// CHECK-LABEL: smear_ proc
// CHECK-NOT: rep
// CHECK: smear_ endp
short s[2000], t[2000];
void up(void) { short i; for (i = 0; i < 1920; ++i) s[i] = s[i + 80]; }
void down(void) { short i; for (i = 1919; i >= 0; --i) s[i + 80] = s[i]; }
void smear(void) { short i; for (i = 0; i < 1920; ++i) s[i + 80] = s[i]; }
