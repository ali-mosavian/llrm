// flags: -O2 -march=i486 | -O2 -march=i486 -m32
extern void report(long value);

static long weight(long k)
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

static long route(long op)
{
    switch (op) {
    case 0: return 11;
    case 37: return 7;
    case 148: return 23;
    case 333: return 5;
    case 592: return 19;
    case 925: return 2;
    case 1332: return 13;
    default: return 1;
    }
}

long bench_switch(int n)
{
    long state = 1, sum = 0;
    int i;
    for (i = 0; i < n; i++) {
        long digit;
        state = (state * 75 + 74) % 65537;
        digit = state % 10;
        sum += weight(state % 14) + route(digit * digit * 37);
    }
    return sum;
}

int main(void)
{
    report(bench_switch(2000));
    return 0;
}
