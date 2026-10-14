extern void report(long value);

static int route(int op)
{
    switch (op) {
    case 3: return 11;
    case 17: return 7;
    case 90: return 23;
    case 200: return 5;
    case 1000: return 19;
    case 4097: return 2;
    case 30000: return 13;
    default: return 1;
    }
}
long bench_x_switch_sparse(int n)
{
    static const int ops[] = { 3, 17, 90, 200, 1000, 4097, 30000, 8, 500, 91 };
    int i; long sum = 0; unsigned r = 7;
    for (i = 0; i < n; ++i) { r = r * 1103515245u + 12345u; sum += route(ops[(r >> 16) % 10]); }
    return sum;
}

int main(void)
{
    report(bench_x_switch_sparse(2000));
    return 0;
}
