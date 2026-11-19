// flags: -O2 | -Os
/* At m16 a switch's lookup table and jump table follow their procedure in the code segment and are read through cs:; every
   selector, the default and the ones either side of the range must give the answer the table in DS gave. */
extern void report(long value);

long weight(long k)
{
    switch (k) {
    case 0: return 3;
    case 1: return 14;
    case 2: return 15;
    case 3: return 92;
    case 4: return 65;
    case 5: return 35;
    case 6: return 89;
    case 7: return 79;
    case 8: return 32;
    case 9: return 38;
    case 10: return 46;
    case 11: return 26;
    default: return 1;
    }
}

long step(int op, long acc)
{
    switch (op) {
    case 0: acc += 3; break;
    case 1: acc *= 2; break;
    case 2: acc -= 7; break;
    case 3: acc ^= 0x55; break;
    case 4: acc = acc << 1 | 1; break;
    case 5: acc /= 3; break;
    default: acc += 1000; break;
    }
    return acc;
}

int main(void)
{
    long k, acc = 5;
    for (k = -2; k < 15; k++)
        report(weight(k));
    for (k = -1; k < 8; k++) {
        acc = step((int)k, acc);
        report(acc);
    }
    return 0;
}
