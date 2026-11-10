// flags: -O0 | -O2
/* A byte stored over a word constant: the byte at the lowest address is the word's high byte on a big-endian target. DSE merged the two stores little-endian, so m68k -O2 printed 0 for 1. */
extern void report(long v);

__UINT32_TYPE__ w; /* 32 bits on every target: a 16-bit `unsigned` made the compare below 0 on z80 */

int main(void)
{
    unsigned one = 1; /* its width does not matter: only the byte order is read */
    int big = *(unsigned char *)&one == 0;
    w = 0x01020304ul;
    *(unsigned char *)&w = 9;
    report(w == (big ? 0x09020304ul : 0x01020309ul));
    return 0;
}
