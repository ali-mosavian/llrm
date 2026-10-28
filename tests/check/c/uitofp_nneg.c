// RUN-PENDING: llrm-cc %s -m32 -Os -march=i486 --isel=dag -S -o /dev/stdout
// `(float)a` of an `unsigned short` was a zero-extended 64-bit buffer and `fild qword`; the pin's
// combineUIntToFP makes the nonnegative conversion signed: `fild dword` of the 32-bit value.
float g(unsigned short a)
{
    return (float)a * 1.5f;
}
// CHECK-LABEL: g_ proc
// CHECK-NOT: fild qword
// CHECK: fild dword
