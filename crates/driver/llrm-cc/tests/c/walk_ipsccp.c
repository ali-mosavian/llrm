/* A callee whose argument is bounded only once the walk has inlined the
   caller's helper into the caller. */
static int sorted[1024];

__attribute__((noinline)) static void sort(int lo, int hi) {
    for (int i = lo; i < hi; i++)
        if (sorted[i] > sorted[i + 1]) {
            int t = sorted[i];
            sorted[i] = sorted[i + 1];
            sorted[i + 1] = t;
        }
}

static inline int clamp(int n) { return n < 1023 ? n : 1023; }

void run(unsigned n, unsigned m) {
    sort(0, clamp(n & 1023));
    sort(0, clamp(m & 2047));
}
