// flags: -O0 | -O2
// targets: x86-m16
// Constant indexes past 32 KB into a near and a far array: the signed i16 index is the same address modulo 64 KB.
extern void report(long v);
char far big[40000];
char near nbig[40000];
int main(void)
{
    big[39000] = 5; nbig[39000] = 6; big[39999] = 7; nbig[39999] = 8;
    report(big[39000] + nbig[39000] * 10 + big[39999] * 100 + nbig[39999] * 1000);
    return 0;
}
