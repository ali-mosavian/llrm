// RUN: llrm-cc %s -Os -w -march=i386 -S -o /dev/stdout
// Two backward string moves take ES as an input: one load serves both (late cleanup keeps it, the block
// lists no segment register), where each loaded it (`push ds; pop es`, 2 B each) or the second reloaded it.
// CHECK-LABEL: {{_f(@3)?}} proc
// CHECK: pop es
// CHECK: {{db 0fdh,0f3h,066h,0a5h.*,0fch}}
// CHECK-NOT: pop es
// CHECK: {{db 0fdh,0f3h,066h,0a5h.*,0fch}}
// CHECK: {{_f(@3)?}} endp
static short a[2000];
void f(void)
{
    short i;
    for (i = 1919; i >= 0; --i) a[i + 80] = a[i];
    for (i = 1919; i >= 0; --i) a[i + 40] = a[i];
}
