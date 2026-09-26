static short tile[64][64];

long tile_sum(short *width, short *height, short *dx, short *dy)
{
    long total;
    short x, y;

    total = 0;
    for (y = 0; y < *height; ++y)
        for (x = 0; x < *width; ++x)
            total += tile[(y + *dy) & 63][(x + *dx) & 63];
    return total;
}
