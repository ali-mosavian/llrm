/* The init loop of object_extent_loop_f32: float lanes from an int counter. */
extern void report(float *);
static float A[12][12];
int main(void)
{
    int i, j;

    for (i = 0; i < 12; i++)
        for (j = 0; j < 12; j++)
            A[i][j] = (i + j) * 0.5f;
    report(A[0]);
    return 0;
}
