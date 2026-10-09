/* LINE: a line, a box or a filled box between the two points (QB rt/grline.asm
   B$LINE). */
#include "gfx.h"

enum { SOLID = -1, LINE_STYLE = 0, BOX = 1, FILLED_BOX = 2 };

/* The pixel pattern of a style: bit by bit along the line, 16 to a turn. */
static unsigned pattern;
static unsigned phase;

static void dot(int x, int y, byte color)
{
    if (pattern & (0x8000u >> (phase++ & 15)))
        gfx_plot(x, y, color, OP_SET);
}

/* The line is always drawn from its left end (its top, if it is upright), as
   QB's are, which matters where a pixel is half way between two.

   The line, stepped along its longer side; the other coordinate is the true one
   plus three quarters of a pixel, cut down to a whole pixel, which is where
   QB's lines put their pixels (measured on its output). */
static void line(int x1, int y1, int x2, int y2, byte color, int solid)
{
    long dx, dy;
    int step_x, step_y;
    long major, minor;
    long k;

    if (x1 > x2 || (x1 == x2 && y1 > y2)) {
        int swap = x1;

        x1 = x2;
        x2 = swap;
        swap = y1;
        y1 = y2;
        y2 = swap;
    }
    dx = x2 > x1 ? x2 - x1 : x1 - x2;
    dy = y2 > y1 ? y2 - y1 : y1 - y2;
    step_x = x2 > x1 ? 1 : -1;
    step_y = y2 > y1 ? 1 : -1;
    major = dx > dy ? dx : dy;
    minor = dx > dy ? dy : dx;

    if (y1 == y2 && solid) {
        gfx_hspan(x1, x2, y1, color, OP_SET);
        return;
    }
    for (k = 0; k <= major; k++) {
        long other = (4 * minor * k + 3 * major) / (4 * major);
        int x = dx > dy ? x1 + (int)k * step_x : x1 + (int)other * step_x;
        int y = dx > dy ? y1 + (int)other * step_y : y1 + (int)k * step_y;

        if (solid)
            gfx_plot(x, y, color, OP_SET);
        else
            dot(x, y, color);
    }
}

/* B$LINE: color (-1 for the foreground), style (-1 for solid), and how: a line,
   B for a box, BF for a filled one. */
void gfx_line_between(int color, int style, int how)
{
    byte c = gfx_color(color);
    int solid = style == SOLID, y;

    pattern = (unsigned)style;
    phase = 0;
    if (how == FILLED_BOX) {
        int top = gfx_y1 < gfx_y2 ? gfx_y1 : gfx_y2;
        int bottom = gfx_y1 < gfx_y2 ? gfx_y2 : gfx_y1;

        for (y = top; y <= bottom; y++)
            gfx_hspan(gfx_x1, gfx_x2, y, c, OP_SET);
    } else if (how == BOX) {
        line(gfx_x1, gfx_y1, gfx_x2, gfx_y1, c, solid);
        line(gfx_x2, gfx_y1, gfx_x2, gfx_y2, c, solid);
        line(gfx_x2, gfx_y2, gfx_x1, gfx_y2, c, solid);
        line(gfx_x1, gfx_y2, gfx_x1, gfx_y1, c, solid);
    } else {
        line(gfx_x1, gfx_y1, gfx_x2, gfx_y2, c, solid);
    }
}
/* B$LINE */
void B_LINE(int color, int style, int how)
{
    gfx_line_between(color, style, how);
}
#pragma aux B_LINE "B$LINE"
