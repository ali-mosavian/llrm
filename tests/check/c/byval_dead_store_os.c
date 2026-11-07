// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -Os -S -o /dev/stdout
// A store into a byval argument just before the return is dead: the copy is the callee's own and nothing reads it
// again (DSEState's constructor treats byval and inalloca arguments as allocas). It stayed, `mov [esp+16], 99`, and
// kept the copy at every inlined call (byval_struct m32 -Os 135 B, pin clang 112).
// CHECK-LABEL: _f proc
// CHECK-NOT: 99
// CHECK: inc eax
// CHECK: _f endp
struct big { long a[10]; };

long f(struct big b, long x)
{
    b.a[3] = 99;
    return x + 1;
}
