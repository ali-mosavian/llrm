// RUN: llrm-cc %s -m32 -mabi=sysv -Os -march=i486 --isel=dag -S -o /dev/stdout
// i8 UDIVREM was CannotSelect under --isel=dag (SD302); then the emitter rejected DIV8. The pin widens into AX and divides with DIV8r/m.
unsigned char ud(unsigned char a, unsigned char b) { return a / b; }
void both(unsigned char a, unsigned char b, unsigned char *q, unsigned char *r) { *q = a / b; *r = a % b; }
// CHECK-LABEL: _ud proc
// CHECK: div byte ptr
// CHECK-NOT: cwd
// CHECK-LABEL: _both proc
// CHECK: div byte ptr
// CHECK: shr {{[a-d]x}}, 8
// The quotient is stored from AL after the remainder's shift, so AX was copied first.
// CHECK: mov byte ptr [{{e[a-d]x}}], al
