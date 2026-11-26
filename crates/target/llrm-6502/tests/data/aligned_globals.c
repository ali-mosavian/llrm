// Over-aligned globals of each section, an odd-sized one before each.
char odd_data[3] = {1, 2, 3};
__attribute__((aligned(16))) char data16[5] = {1};
char odd_bss[3];
__attribute__((aligned(16))) char bss16[5];
const char odd_const[3] = {1, 2, 3};
__attribute__((aligned(16))) const char const16[5] = {1};
__attribute__((aligned(2))) char data2[1] = {1};

__attribute__((noinline)) static unsigned low(const volatile void *p, unsigned mask)
{
    return (unsigned)p & mask;
}

int main(void)
{
    odd_bss[0] = odd_data[0] + odd_const[0];
    return (low(data16, 15) != 0) | (low(bss16, 15) != 0) << 1 | (low(const16, 15) != 0) << 2 | (low(data2, 1) != 0) << 3;
}
