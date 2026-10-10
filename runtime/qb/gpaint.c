/* PAINT: fills the area round a point, up to a border colour (QB rt/paint.asm
   B$PAIN).  Spans a row at a time, each span leaving seeds for the rows above
   and below it.  The seeds wait in all the free string space, as QB's queue
   does, so a fill runs out of memory only where QB's would. */
#include "gfx.h"
#include "nhstutil.h"

typedef struct Seed {
    int x, y;
} Seed;

static Seed *seeds;
static unsigned seed_count, seed_room;

static void push(int x, int y)
{
    if (seed_count == seed_room)
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

/* Seeds the row `y` with a seed for each run of open pixels, not of the fill
   colour, in `left` to `right`. */
static void seed_row(int left, int right, int y, int border, int fill)
{
    int x = left, open, closed;

    if (y < 0 || y >= (int)gfx_current->height)
        return;
    while (x <= right && (open = gfx_search(x, right, y, border, fill, 0)) >= 0) {
        push(open, y);
        closed = gfx_search(open, right, y, border, fill, 1);
        if (closed < 0)
            break;
        x = closed + 1;
    }
}

/* B$PAIN: the fill colour and the border colour (-1 for the foreground, and for
   the border the fill colour). */
void B_PAIN(int fill, int border)
{
    byte paint = gfx_color(fill);
    byte edge = border == -1 ? paint : gfx_color(border);
    char *space;
    SD *held;

    seed_count = 0;
    if (!open_pixel(gfx_x1, gfx_y1, edge))
        return;
    seed_room = str_free_bytes() / sizeof(Seed);
    held = str_tmp(seed_room * sizeof(Seed), &space);
    seeds = (Seed *)space;
    push(gfx_x1, gfx_y1);
    while (seed_count) {
        Seed seed = seeds[--seed_count];
        int left = seed.x, right = seed.x;

        if (!open_pixel(seed.x, seed.y, edge)
            || gfx_pixel(seed.x, seed.y) == paint)
            continue;
        left = gfx_search(seed.x, 0, seed.y, edge, edge, 1);
        left = left < 0 ? 0 : left + 1;
        right = gfx_search(seed.x, (int)gfx_current->width - 1, seed.y, edge, edge, 1);
        right = right < 0 ? (int)gfx_current->width - 1 : right - 1;
        gfx_hspan(left, right, seed.y, paint, OP_SET);
        seed_row(left, right, seed.y - 1, edge, paint);
        seed_row(left, right, seed.y + 1, edge, paint);
    }
    str_tmp_free(held);
}
#pragma aux B_PAIN "B$PAIN"
