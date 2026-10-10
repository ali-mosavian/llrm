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

typedef struct Point {
    long x, y;
} Point;

/* Which sides of the screen a point is outside of: 1 left, 2 right, 4 above,
   8 below. */
static int outside(const Point *p)
{
    return (p->x < 0) | (p->x >= (long)gfx_current->width) << 1
         | (p->y < 0) << 2 | (p->y >= (long)gfx_current->height) << 3;
}

/* The other coordinate of the line from (`along_from`, `other_from`) to
   (`along_to`, `other_to`) where the first coordinate is `edge`: the exact
   value, rounded to the nearest whole number, a half going away from 0 from the
   offset it is measured from. */
static long cut(long along_from, long other_from, long along_to, long other_to, long edge)
{
    long numerator = (other_to - other_from) * (edge - along_from);
    long denominator = along_to - along_from;
    long twice, sign = 1;

    if (denominator < 0) {
        denominator = -denominator;
        numerator = -numerator;
    }
    if (numerator < 0) {
        numerator = -numerator;
        sign = -1;
    }
    twice = (2 * numerator + denominator) / (2 * denominator);
    return other_from + sign * twice;
}

/* Cuts the line to the screen by moving the end that is outside onto the edge
   it is past, taking the sides in the order left, right, above, below, until
   both ends are inside.  Which end ends up first is the one the line is drawn
   from.  False when none of it is on the screen. */
static int clip(Point *first, Point *last)
{
    int first_out = outside(first), last_out = outside(last);

    for (;;) {
        if (!(first_out | last_out))
            return 1;
        if (first_out & last_out)
            return 0;
        if (!first_out) {
            Point swap = *first;

            *first = *last;
            *last = swap;
            first_out = last_out;
            last_out = 0;
        }
        if (first_out & 3) {
            long edge = first_out & 1 ? 0 : (long)gfx_current->width - 1;

            first->y = cut(first->x, first->y, last->x, last->y, edge);
            first->x = edge;
        } else {
            long edge = first_out & 4 ? 0 : (long)gfx_current->height - 1;

            first->x = cut(first->y, first->x, last->y, last->x, edge);
            first->y = edge;
        }
        first_out = outside(first);
    }
}

/* QB's line.  It is drawn from the first point (after clipping), or from the
   other when that is the one on the left, stepping along the longer side.  A
   decision value starts at four times the shorter side less the longer, and
   after each pixel the other coordinate moves too when the value has reached 0
   (the value then takes four times the shorter less the longer, else four times
   the shorter): the pixels are where QB's are, and a line style's bits fall on
   the same ones. */
static void line(int x1, int y1, int x2, int y2, byte color, int solid)
{
    Point first, last;
    long dx, dy, major, minor, decision, k;
    int step_y, x, y;

    first.x = x1;
    first.y = y1;
    last.x = x2;
    last.y = y2;
    if (!clip(&first, &last))
        return;
    if (first.x > last.x) {
        Point swap = first;

        first = last;
        last = swap;
    }
    x = (int)first.x;
    y = (int)first.y;
    dx = last.x - first.x;
    dy = last.y > first.y ? last.y - first.y : first.y - last.y;
    step_y = last.y > first.y ? 1 : -1;
    major = dx > dy ? dx : dy;
    minor = dx > dy ? dy : dx;
    if (first.y == last.y && solid) {
        gfx_hspan(x, (int)last.x, y, color, OP_SET);
        return;
    }
    decision = 4 * minor - major;
    for (k = 0; k <= major; k++) {
        if (solid)
            gfx_plot(x, y, color, OP_SET);
        else
            dot(x, y, color);
        if (decision < 0) {
            decision += 4 * minor;
        } else {
            decision += 4 * (minor - major);
            if (dx > dy)
                y += step_y;
            else
                x++;
        }
        if (dx > dy)
            x++;
        else
            y += step_y;
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
        /* the edges in QB's order: bottom, top, right, left */
        line(gfx_x1, gfx_y2, gfx_x2, gfx_y2, c, solid);
        line(gfx_x1, gfx_y1, gfx_x2, gfx_y1, c, solid);
        line(gfx_x2, gfx_y1, gfx_x2, gfx_y2, c, solid);
        line(gfx_x1, gfx_y1, gfx_x1, gfx_y2, c, solid);
    } else {
        line(gfx_x1, gfx_y1, gfx_x2, gfx_y2, c, solid);
    }
}
/* B$LINE */
void B_LINE(short color, short style, short how)
{
    gfx_line_between(color, style, how);
}
#pragma aux B_LINE "B$LINE"
