// RUN: llrm-cc %s -m16 -march=i386 -Os --isel=dag -S -o /dev/stdout
// Float fields passed to a call next to a computed float (QCport demo.c, `demo_lerp_angle(a->pitch, b->pitch, frac)`): the loads are
// shared in the DAG, so the cast to bits stayed a float store and the arguments went `fld; sub sp; fstp [bx+k]`. Read again as integers
// they are pushes of memory (DEVIATE SD256); what was ordered after the float load stays ordered after the integer load.
// CHECK-LABEL: _h@3 proc
// CHECK-NOT: fld
// CHECK: push dword ptr [bp+6]
// CHECK-NEXT: push dword ptr [bx]
// CHECK-NEXT: push dword ptr [si]
// CHECK-NEXT: call far ptr _lerp@3
// CHECK: _h@3 endp
struct Angles {
    float pitch, yaw;
};
extern float lerp(float, float, float);
extern float sink;
void h(struct Angles *a, struct Angles *b, float frac)
{
    sink = lerp(a->pitch, b->pitch, frac);
    a->pitch = 0;
}
