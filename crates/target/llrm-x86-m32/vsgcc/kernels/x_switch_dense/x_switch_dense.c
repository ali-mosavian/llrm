extern void report(long value);

static int weight(int k)
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
long bench_x_switch_dense(int n)
{
    int i; long sum = 0; unsigned r = 5;
    for (i = 0; i < n; ++i) { r = r * 1103515245u + 12345u; sum += weight((int)((r >> 16) % 14)); }
    return sum;
}

int main(void)
{
    report(bench_x_switch_dense(2000));
    return 0;
}
