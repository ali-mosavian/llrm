// RUN: llrm-cc %s -m32 -mabi=sysv -Os -march=i486 --isel=dag -S -o /dev/stdout
// The inner loop of a table-driven crc: `c & 1` and `c >> 1` both read c, and c is dead after them. The pin's pre-RA scheduler runs after two-address and
// the coalescer, so it sees both copies and orders the shr's copy first: the `and` is then in place on c's register and one copy is left
// (x_crc32 -Os, +10% instructions when the scheduler ran before two-address: a copy for each and a `mov` of the masked value back).
unsigned long table[256];
void f(void)
{
    unsigned long c;
    int i, k;
    for (i = 0; i < 256; ++i) {
        c = (unsigned long)i;
        for (k = 0; k < 8; ++k) c = (c & 1) ? 0xEDB88320ul ^ (c >> 1) : c >> 1;
        table[i] = c;
    }
}
// CHECK-LABEL: _f proc
// CHECK: and e{{..}}, 1
// CHECK-NEXT: shr e{{..}}, 1
// CHECK-NEXT: neg e{{..}}
// CHECK-NEXT: and e{{..}}, -306674912
// CHECK-NEXT: xor e{{..}}, e{{..}}
// CHECK-NEXT: dec
// CHECK: _f endp
