extern void report(long value);

static int keys[256], vals[256];
static unsigned hash(int k) { unsigned h = (unsigned)k * 2654435761u; return h >> 24; }
long bench_x_hash(int n)
{
    int i, slot; long sum = 0;
    for (i = 0; i < 256; ++i) keys[i] = -1;
    for (i = 0; i < n; ++i) {
        int k = (i * 97) % 211; slot = (int)hash(k);
        while (keys[slot] != -1 && keys[slot] != k) slot = (slot + 1) & 255;
        if (keys[slot] == -1) { keys[slot] = k; vals[slot] = i; } else vals[slot] += i;
    }
    for (i = 0; i < 256; ++i) if (keys[i] != -1) sum += vals[i] ^ keys[i];
    return sum;
}

int main(void)
{
    report(bench_x_hash(600));
    return 0;
}
