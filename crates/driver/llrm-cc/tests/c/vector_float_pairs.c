/* Two tables of doubles filled from the loop counters, a row of twelve unrolled into pairs. clang -O2 and the pin make
   each pair one `mulpd` of two scalar conversions; ours kept every lane scalar because the pair's cast subtree cost
   more than gathering its scalars and nothing cut it (object_extent_loop, +5.6% over clang at -O2). */
double A[12][12], B[12][12];

void init(void)
{
    for (int i = 0; i < 12; i++)
        for (int j = 0; j < 12; j++) {
            A[i][j] = (i + j) * 0.5;
            B[i][j] = (i - j) * 0.25 + 1.0;
        }
}
