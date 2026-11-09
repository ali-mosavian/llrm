// flags: -O0 | -O2
/* A byte stored over a word constant: the byte at the lowest address is the word's high byte on a big-endian target. DSE merged the two stores little-endian, so m68k -O2 printed 0 for 1. */
extern void report(long v);

unsigned w;

int main(void)
{
    unsigned one = 1;
    int big = *(unsigned char *)&one == 0;
    w = 0x01020304u;
    *(unsigned char *)&w = 9;
    report(w == (big ? 0x09020304u : 0x01020309u));
    return 0;
}
