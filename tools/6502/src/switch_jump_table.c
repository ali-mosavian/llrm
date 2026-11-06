// flags: -O0 | -O2 | -Os | -O2 -fno-unroll-loops -fno-peel-loops
/* A dense switch whose cases each do their own work is a jump table (gcc's from 5 cases): the index goes through a table of
   code addresses. The hole at 6, the selectors either side of the range and a byte selector are the default. */
extern void report(long value);

static long step(int op, long acc)
{
    switch (op) {
    case 0: acc += 3; break;
    case 1: acc *= 2; break;
    case 2: acc -= 7; break;
    case 3: acc ^= 0x55; break;
    case 4: acc = acc << 1 | 1; break;
    case 5: acc /= 3; break;
    case 7: acc += acc >> 2; break;
    case 8: acc = -acc; break;
    case 9: acc &= 0xff; break;
    default: acc += 1000; break;
    }
    return acc;
}

static long byte(unsigned char op, long acc)
{
    switch (op) {
    case 250: return acc + 1;
    case 251: return acc * 3;
    case 252: return acc - 4;
    case 253: return acc ^ 9;
    case 254: return acc | 16;
    case 255: return acc >> 1;
    default: return -acc;
    }
}

int main(void)
{
    long acc = 5;
    int op;
    for (op = -3; op < 14; op++) {
        acc = step(op, acc);
        report(acc);
    }
    for (op = 248; op < 258; op++)
        report(byte((unsigned char)op, 100 + op));
    return 0;
}
