// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -O2 -S -o /dev/stdout
// A variable-length copy at m32 -O1/-O2 is `rep movsd` of n/4 and `rep movsb` of n&3. The second block started over at
// the copy's first byte (`mov esi, offset _s+3; mov edi, offset _d+1` between them) instead of where the first stopped,
// so the last n&3 bytes were never copied: n = 17 left d[17] unwritten (tests/run/c idiom_edges line 13, idioms line 42).
char s[40], d[40];
__attribute__((noinline)) void copy(short n)
{
    short i;
    for (i = 0; i < n; ++i) d[i + 1] = s[i + 3];
}
// CHECK-LABEL: copy proc
// CHECK: rep movsd
// CHECK-NOT: mov esi, offset
// CHECK-NOT: mov edi, offset
// CHECK: rep movsb
