// RUN: llrm-c %s -m32 -mabi=sysv -Os -march=i486 --isel=dag -S -o /dev/stdout
// i8 UDIVREM/SDIVREM was CannotSelect under --isel=dag (SD302). The pin widens into AX and divides with DIV8r/IDIV8r.
unsigned char ud(unsigned char a, unsigned char b) { return a / b; }
signed char sr(signed char a, signed char b) { return a % b; }
// CHECK-LABEL: _ud proc
// CHECK: movzx
// CHECK: div {{[a-d]l}}
// CHECK-LABEL: _sr proc
// CHECK: movsx
// CHECK: idiv {{[a-d]l}}
// CHECK: movzx {{.*}}ah
