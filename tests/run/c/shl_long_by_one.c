/* A 32-bit value shifted left by one, constant count: the carry out of the low word must reach the high word
   (shl x,1 is add x,x, an add hl,hl and adc hl,de). Values with bit 15, bit 31 and the carry chain. */
extern void report(long value);
typedef __UINT32_TYPE__ u32;
typedef __INT32_TYPE__ i32;

__attribute__((noinline)) static u32 twice(u32 x) { return x << 1; }
__attribute__((noinline)) static i32 twice_signed(i32 x) { return x << 1; }
/* a second caller, so neither is a single-use routine inlined into its caller */
__attribute__((noinline)) static u32 four(u32 x) { return twice(twice(x)); }

int main(void)
{
    static const u32 xs[] = {0u, 1u, 0x00007FFFu, 0x00008000u, 0x0000FFFFu, 0x00010000u, 0x40000000u, 0x80000000u, 0xFFFFFFFFu, 0x12348765u};
    int i;
    for (i = 0; i < 10; i++) {
        report((long)(i32)twice(xs[i]));
        report((long)twice_signed((i32)xs[i]));
        report((long)(i32)four(xs[i]));
    }
    return 0;
}
