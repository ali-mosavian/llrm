/* Four copies of one inlined loop, each ending in the same three stores of
 * different values: at -Os they are one copy after the switch (gcc's
 * -fcrossjumping), not four. */
typedef struct {
    int a, b, c;
} S;

[[gnu::always_inline]] static inline void run(S *const into, const int n, const int w)
{
    int a = into->a, b = into->b;

    for (int i = 0; i < n; i++) {
        a += w * i;
        b ^= a >> w;
    }
    into->a = a;
    into->b = b;
    into->c = a + b + w;
}

void render(S *const s, const int n, const int k)
{
    switch (k) {
    case 0: run(s, n, 1); break;
    case 1: run(s, n, 2); break;
    case 2: run(s, n, 3); break;
    default: run(s, n, 5);
    }
}
