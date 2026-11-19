// RUN: llrm-cc %s -m32 -mabi=sysv -O2 -march=i486 -S -o /dev/stdout
// The outer loop of 4 reads a constant table at its counter: each copy shifts a constant, which folds. Counting only the bytes of the copy kept the nest rolled (tests/run shift_int64_variable: +313 B, UP1).
// CHECK-LABEL: _shifts proc
// CHECK-NOT: jne
// CHECK-NOT: je
// CHECK: _shifts endp
static const unsigned long long values[4] = { 0x0123456789ABCDEFULL, 0xFFFFFFFFFFFFFFFFULL, 0x8000000000000001ULL, 0x00000000FFFF0001ULL };
unsigned long shifts(void)
{
    unsigned long acc = 0;
    int i;
    unsigned c;
    for (i = 0; i < 4; ++i)
        for (c = 0; c < 64; ++c) acc = acc * 31UL + (unsigned long)(values[i] << c) + (unsigned long)(values[i] >> c);
    return acc;
}
