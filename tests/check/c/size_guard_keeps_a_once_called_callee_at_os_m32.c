// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -Os -S -o /dev/stdout
// At m32 -Os a callee with one site was copied into its caller although the copy grows the code: crc 279 B against
// 230 B with the call kept (gcc keeps it; the pin copies it by its cost model, DEVIATE SZ2).
extern void report(long value);

unsigned long crc(unsigned long salt)
{
    static const unsigned char data[] = "123456789";
    unsigned long c = 0xffffffffUL ^ salt;
    unsigned short i, bit;
    for (i = 0; i < 9; ++i) {
        c ^= data[i];
        for (bit = 0; bit < 8; ++bit)
            c = (c >> 1) ^ (0xedb88320UL & (0UL - (c & 1UL)));
    }
    return c ^ 0xffffffffUL;
}

int main(void)
{
    report((long)crc(0));
    return 0;
}
// CHECK-LABEL: _main proc
// CHECK: call _crc
