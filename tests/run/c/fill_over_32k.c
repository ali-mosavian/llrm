// flags: -O2 -march=i486
// targets: x86-m16
/* A fill of more than 32767 bytes. The loop became one memset of 40000, a 16-bit count that read as -25536 and stored nothing:
   the function was empty and the sum read zeros. */
extern void report(long value);
char big[45000];
int main(void)
{
    unsigned i;
    unsigned long sum = 0;
    for (i = 0; i < 40000u; i++) big[i] = 2;
    for (i = 0; i < 40000u; i++) sum += big[i];
    report((long)sum);
    return 0;
}
