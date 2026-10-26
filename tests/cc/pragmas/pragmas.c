// RUN: %clang_cc1 -triple i386-pc-none-code16 -fborland-extensions -std=gnu89 -emit-llvm -o - %s | FileCheck %s
// Watcom's `#pragma aux NAME "LINK"` is the link name of NAME, as an asm label;
// `data_seg("NAME", "CLASS")` and `code_seg` are the Microsoft section pragmas,
// the class being the linker's.

void qb_sincos(void);
#pragma aux qb_sincos "B$SINCOS"
extern int counter;
#pragma aux counter "CNT$"
int counter;

// CHECK: @in_xib = {{.*}}global i16 3, section "XIB"
// CHECK: @plain = {{.*}}global i16 4, align
// CHECK: @"\01CNT$" = {{.*}}global i16
#pragma data_seg("XIB", "DATA")
int in_xib = 3;
#pragma data_seg()
int plain = 4;

// CHECK: define {{.*}}void @use(){{.*}}section "HOT_TEXT"
// CHECK: call addrspace(1) void @"\01B$SINCOS"()
#pragma code_seg("HOT_TEXT", "CODE")
void use(void) { qb_sincos(); counter++; }
#pragma code_seg()
// CHECK: define {{.*}}void @cold() addrspace(1) #
void cold(void) {}
