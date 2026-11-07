// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -Os -S -o /dev/stdout
// A double filled by two i32 stores and read whole: SROA widens the slot to i64 and the pun folds. The port refused a
// float/integer reinterpretation off 64-bit-register targets (isel does it through the stack at m32, as the pin
// assumes), so `alloca double` + fld stayed (float_bits -Os 132 B, pin clang 93).
// CHECK-LABEL: _f proc
// CHECK-NOT: fld
// CHECK: mov eax, 12
// CHECK: _f endp
union D { double d; unsigned w[2]; };

static double join(unsigned hi, unsigned lo)
{
    union D u;
    u.w[1] = hi;
    u.w[0] = lo;
    return u.d;
}

long f(void)
{
    return (long)(join(0x40100000u, 0) * 3.0);
}
