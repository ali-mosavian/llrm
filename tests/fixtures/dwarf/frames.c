volatile int sink;

__attribute__((noinline)) int leaf(int a, int b, int c, int d, int e, int f)
{
    sink = a + f;
    return a + b + c + d + e + f;
}

/* noinline: at -O2 both calls fold to a constant and nothing is left to walk. No variable of its own: no frame register, so its stack pointer moves with each argument pushed. */
__attribute__((noinline)) int middle(void)
{
    return leaf(1, 2, 3, 4, 5, 6) + leaf(2, 3, 4, 5, 6, 7);
}

/* The store after the call keeps `main` a frame: `return middle();` is a tail call at -O2. */
int main(void)
{
    int result = middle();
    sink = result;
    return result;
}
