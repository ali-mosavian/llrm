/* The box fills of runtime/qb/dos/gfxdev.c on the screen, every mode and every operation: a random box over random pixels leaves
   what the operation says of each pixel, and the pixels around it as they were; so does a dot of the same fill.  Prints 0, or the first case that differed. */
#include "gfxdev.h"

extern void report(long value);

enum { REGION_W = 48, REGION_H = 24, ROUNDS = 120 };

static unsigned long rng = 12345;
static unsigned next(void)
{
    rng = rng * 1103515245UL + 12345UL;
    return (unsigned)(rng >> 16) & 0x7FFF;
}

static unsigned before[REGION_W * REGION_H];

int main(void)
{
    static const unsigned modes[] = { 0x13, 4, 6, 0x10, 0x0D };
    static const unsigned colors[] = { 256, 4, 2, 16, 16 };
    unsigned m, round, x, y;

    for (m = 0; m < sizeof(modes) / sizeof(modes[0]); m++) {
        gd_set_mode(modes[m]);
        for (round = 0; round < ROUNDS; round++) {
            unsigned operation = round & 3, color = next() % colors[m];
            unsigned bx = next() % REGION_W, by = next() % REGION_H;
            unsigned count = 1 + next() % (REGION_W - bx), rows = 1 + next() % (REGION_H - by);
            GdFill fill;

            for (y = 0; y < REGION_H; y++)
                for (x = 0; x < REGION_W; x++) {
                    gd_plot(x, y, next() % colors[m], 0);
                    before[y * REGION_W + x] = gd_read(x, y);
                }
            gd_fill_select(&fill, color, operation);
            fill.box(&fill, bx, by, count, rows);
            for (y = 0; y < REGION_H; y++)
                for (x = 0; x < REGION_W; x++) {
                    unsigned old = before[y * REGION_W + x], want = old;

                    if (x >= bx && x < bx + count && y >= by && y < by + rows)
                        want = (operation == 0 ? color : operation == 1 ? old & color : operation == 2 ? old | color : old ^ color) & (colors[m] - 1);
                    if (gd_read(x, y) != want) {
                        report((long)modes[m] * 1000000L + (long)operation * 100000L + (long)(bx + by * 100) + 1);
                        return 0;
                    }
                }
            /* and a few dots of the same fill: the pixel as the operation says, its neighbours as they were */
            for (x = 0; x < 6; x++) {
                unsigned dx = 1 + next() % (REGION_W - 2), dy = next() % REGION_H;
                unsigned left = gd_read(dx - 1, dy), old = gd_read(dx, dy), right = gd_read(dx + 1, dy);
                unsigned want = (operation == 0 ? color : operation == 1 ? old & color : operation == 2 ? old | color : old ^ color) & (colors[m] - 1);

                gd_dots_begin(&fill);
                fill.dot(&fill, dx, dy);
                gd_dots_end(&fill);
                if (gd_read(dx, dy) != want || gd_read(dx - 1, dy) != left || gd_read(dx + 1, dy) != right) {
                    report((long)modes[m] * 1000000L + (long)operation * 100000L + (long)(dx + dy * 100) + 50000L);
                    return 0;
                }
            }
        }
    }
    report(0);
    return 0;
}
