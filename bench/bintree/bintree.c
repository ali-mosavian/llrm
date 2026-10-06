// flags: -O2 --cpu 486 | -O2 --cpu 486 --target x86-code32
extern void report(long value);

#define NODES 300

/* Node i is t[3i] key, t[3i+1] left, t[3i+2] right; index 0 is nil and node 1 the root. */
static void insert(int *t, int at, int node)
{
    int side = t[3 * node] < t[3 * at] ? 1 : 2;

    if (t[3 * at + side] == 0) t[3 * at + side] = node;
    else insert(t, t[3 * at + side], node);
}

/* Sum of key x depth (root 1) over the subtree at `at`, walked in order. */
static long walk(const int *t, int at, int depth)
{
    if (at == 0) return 0;
    return walk(t, t[3 * at + 1], depth + 1) + (long)t[3 * at] * depth + walk(t, t[3 * at + 2], depth + 1);
}

long bench_bintree(unsigned short seed)
{
    int t[3 * (NODES + 1)];
    int i;
    unsigned short x = seed;

    for (i = 0; i < 3 * (NODES + 1); ++i) t[i] = 0;
    for (i = 1; i <= NODES; ++i) {
        x = x * 25173u + 13849u;
        t[3 * i] = x & 0xFFF;
        if (i > 1) insert(t, 1, i);
    }
    return walk(t, 1, 1);
}

int main(void)
{
    report(bench_bintree(1));
    return 0;
}
