typedef struct {
    short x;
    short y;
} Pair;

long parity_kernel(void)
{
    Pair points[8];
    long total;
    short index;

    total = 17;
    for (index = 0; index < 8; ++index) {
        points[index].x = index * 3 + 1;
        points[index].y = 29 - index * 2;
    }
    for (index = 0; index < 8; ++index)
        total += (long)points[index].x * points[index].y;
    return total;
}
