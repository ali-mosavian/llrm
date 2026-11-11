// RUN: llrm-cc %s -m32 -mabi=sysv -O2 -march=i486 -fllrm-stable-loads -S -o /dev/stdout
// Eight loads of a global held across a call that only reads memory: the call frame's `sub esp`/`add esp` pseudos were taken for writes of
// memory, so no load was a stable cell and three were spilled (a store and a reload each) instead of read again from the global.
// A call that may write memory still keeps its spills.
extern int g[16];
extern int pure_call(int) __attribute__((pure));
extern int plain_call(int);
int f(int n)
{
    int a0 = g[0], a1 = g[1], a2 = g[2], a3 = g[3], a4 = g[4], a5 = g[5], a6 = g[6], a7 = g[7];
    int r = pure_call(n);
    return r + a0 * 3 + a1 * 5 + a2 * 7 + a3 * 9 + a4 * 11 + a5 * 13 + a6 * 15 + a7 * 17;
}
int h(int n)
{
    int a0 = g[0], a1 = g[1], a2 = g[2], a3 = g[3], a4 = g[4], a5 = g[5], a6 = g[6], a7 = g[7];
    int r = plain_call(n);
    return r + a0 * 3 + a1 * 5 + a2 * 7 + a3 * 9 + a4 * 11 + a5 * 13 + a6 * 15 + a7 * 17;
}
// CHECK-LABEL: _f proc
// CHECK-NOT: mov dword ptr [esp
// CHECK: _f endp
// CHECK-LABEL: _h proc
// CHECK: mov dword ptr [esp
// CHECK: _h endp
