// RUN: llrm-cc %s -m16 -Os -S -o /dev/stdout
// A field walked through an array of 44-byte records at -Os: LSR orders formulae by bytes, so the global's own base takes the field
// offset in the displacement. With the offset folded into a biased base (`offset _V+4`) the zero-trip path got its own copy of the result.
// CHECK-LABEL: _sum@3 proc
// CHECK-NOT: offset _V+4
// CHECK: add {{e[a-d]x}}, dword ptr [bx+4]
// CHECK-NOT: jmp
// CHECK: _sum@3 endp
struct R { long a; long b; char pad[36]; };
extern struct R V[];
long sum(int n)
{
    long s = 0;
    for (int i = 0; i < n; ++i) s += V[i].b;
    return s;
}
