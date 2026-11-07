// RUN: llrm-cc %s -m64 -fno-pic -O1 -S -o /dev/stdout
// ConstantHoisting was not wired: the NaN tests loaded 0xFF000000 twice and joined the flags with setcc/or (sfm16_float_div __gesf2 +57 B). The pin hoists it as base+offset, which makes the join cost 3, so it splits into two cmp+ja.
// CHECK: cmp r{{[0-9a-z]+}}, r{{[0-9a-z]+}}
// CHECK: ja
// CHECK: cmp r{{[0-9a-z]+}}, r{{[0-9a-z]+}}
// CHECK: ja
typedef unsigned long u;
static int cmp(u a, u b)
{
    if ((a << 1) > 0xFF000000ul || (b << 1) > 0xFF000000ul) return -1;
    if (!((a | b) << 1)) return 0;
    long x = (long)a, y = (long)b;
    if (x < 0) x = (long)0x80000000ul - x;
    if (y < 0) y = (long)0x80000000ul - y;
    return x < y ? -1 : x > y;
}
int f(u a, u b) { return cmp(a, b); }
