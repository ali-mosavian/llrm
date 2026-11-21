/* A loop that fills doubles from an int counter: clang -O2 converts two lanes at a time (`cvtdq2pd`, `mulpd`), ours stayed
   scalar (`cvtsi2sd` a lane) because no float lane was priced: object_extent_loop's two init loops, +8.7% over clang. */
double table[1024];

void fill(int base, int n)
{
    for (int j = 0; j < n; j++)
        table[j] = (base + j) * 0.5;
}
