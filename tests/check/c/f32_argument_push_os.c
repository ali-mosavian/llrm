// RUN: llrm-cc %s -m16 -march=i386 -Os --isel=dag -S -o /dev/stdout
// An `f32` parameter passed on to a call went through the x87 stack: `sub sp, 4; mov bx, sp; fld [bp+6]; fstp [bx]` (10 B, a
// register and two x87 moves) where `push dword ptr [bp+6]` is 3 B. At -Os the value that is only a memory load is pushed as its
// bits (DEVIATE SD256); QCport d_sky_pan was 26 B in Borland and 42 B over it.
// CHECK-LABEL: _pan@3 proc
// CHECK-NOT: fstp
// CHECK: push dword ptr [bp+6]
// CHECK: call
extern float g(float);
float pan(float t)
{
    return g(t) * 2.0f;
}
