/* PAINT: fills the area round a point, up to a border colour (QB rt/paint.asm
   B$PAIN).  Spans a row at a time, each span leaving seeds for the rows above
   and below it. */
#include "gfx.h"

enum { SEEDS = 96 };

typedef struct Seed {
    int x, y;
} Seed;

static Seed seeds[SEEDS];
static unsigned seed_count;

static void push(int x, int y)
{
    if (seed_count == SEEDS)
        qb_error(BE_MEMORY);
    seeds[seed_count].x = x;
    seeds[seed_count].y = y;
    seed_count++;
}

/* Whether the pixel can be painted: on the screen, not the border. */
static int open_pixel(int x, int y, int border)
{
    int c = gfx_pixel(x, y);

    return c >= 0 && c != border;
}

/* Seeds the row `y` with a seed for each run of open pixels in `left` to
   `right`. */
static void seed_row(int left, int right, int y, int border, int fill)
{
    int x, inside = 0;

    for (x = left; x <= right; x++) {
        int c = gfx_pixel(x, y);
        int open = c >= 0 && c != border && c != fill;

        if (open && !inside)
            push(x, y);
        inside = open;
    }
}

/* B$PAIN: the fill colour and the border colour (-1 for the foreground, and for
   the border the fill colour). */
void B_PAIN(int fill, int border)
{
    byte paint = gfx_color(fill);
    byte edge = border == -1 ? paint : gfx_color(border);

    seed_count = 0;
    if (!open_pixel(gfx_x1, gfx_y1, edge))
        return;
    push(gfx_x1, gfx_y1);
    while (seed_count) {
        Seed seed = seeds[--seed_count];
        int left = seed.x, right = seed.x;

        if (!open_pixel(seed.x, seed.y, edge)
            || gfx_pixel(seed.x, seed.y) == paint)
            continue;
        while (open_pixel(left - 1, seed.y, edge))
            left--;
        while (open_pixel(right + 1, seed.y, edge))
            right++;
        gfx_hspan(left, right, seed.y, paint, OP_SET);
        seed_row(left, right, seed.y - 1, edge, paint);
        seed_row(left, right, seed.y + 1, edge, paint);
    }
}
#pragma aux B_PAIN "B$PAIN"
