/* CIRCLE: a circle, an ellipse, or an arc of one (QB rt/circle.asm B$CIRC).
   B$CSTT, B$CSTO and B$CASP give the start angle, the end angle and the aspect
   ratio when the statement has them; B$CIRC then draws.  An angle given as a
   negative number also draws a line from the centre to that end of the arc.

   The circle is the midpoint circle of the radius rounded to a whole number,
   its points reflected into the eight octants.  An ellipse is the same circle
   with the offsets on one axis scaled by the aspect ratio, in 8.8 fixed point
   and rounded: the y offsets where the aspect is under 1, the x offsets where
   it is over 1 (the radius then being the y radius).  An arc counts the points
   round the circle, n to an octant, and keeps those from the start's count to
   the end's. */
#include "device.h"
#include "gfx.h"

enum { OCTANTS = 8, FIXED = 256, TURN_COUNTS = 8 };

static double start_angle, end_angle, aspect_ratio;
static int has_start, has_end, has_aspect;

void B_CSTT(float angle)
{
    start_angle = angle;
    has_start = 1;
}

void B_CSTO(float angle)
{
    end_angle = angle;
    has_end = 1;
}

void B_CASP(float ratio)
{
    aspect_ratio = ratio;
    has_aspect = 1;
}

typedef struct Circle {
    int cx, cy;
    long radius;
    long scale;             /* the aspect, in 256ths, of the scaled axis */
    int scale_x;
    long points;            /* n: of an octant */
    byte color;
    int arc;
    unsigned long from, to; /* counts round the circle, from <= to */
    byte outside;           /* the arc is what lies outside from..to */
    byte spoke_from, spoke_to;     /* a line from that end of the arc to the centre */
    byte drawn_from, drawn_to;     /* and whether it has been drawn */
} Circle;

static const double pi = 3.141592653589793;

/* An offset on the scaled axis: its size scaled, then its sign. */
static long scaled(const Circle *c, long offset)
{
    long size = offset < 0 ? -offset : offset;

    size = (size * c->scale + FIXED / 2) / FIXED;
    return offset < 0 ? -size : size;
}

/* How far round the circle an angle is, in counts: the octant it is in gives a
   base, and the sine or cosine of the angle the distance from it. */
static unsigned long count_of(const Circle *c, double angle)
{
    double sine, cosine, along;
    int octant = (int)(angle * 4 / pi);
    long base = 2 * ((octant + 1) / 2) * c->points;

    dev_sincos(&angle, &sine, &cosine);
    /* octants 1, 2, 5 and 6 are measured from the vertical axis */
    along = (((octant + 1) / 2) & 1 ? cosine : sine) * c->radius;
    if (along < 0)
        along = -along;
    if (octant == 1 || octant == 3 || octant == 5 || octant == 7)
        along = -along;
    return (unsigned long)(along + base + 0.5);
}

/* A point of the rim, which has `count` round the circle.  The point at one end
   of an arc that has a line to the centre starts that line, and is not plotted
   itself; the other points are plotted if they are between the ends (or
   outside them, for an arc that runs through 0). */
static void spoke(Circle *c, int x, int y, byte *drawn)
{
    *drawn = 1;
    gfx_x1 = x;
    gfx_y1 = y;
    gfx_x2 = c->cx;
    gfx_y2 = c->cy;
    gfx_line_between(c->color, -1, 0);
}

static void plot(Circle *c, long x, long y, unsigned long count)
{
    int px = c->cx + (int)(c->scale_x ? scaled(c, x) : x);
    int py = c->cy + (int)(c->scale_x ? y : scaled(c, y));
    int between = 0, endpoint = 0;

    if (c->arc) {
        if (count == c->from) {
            if (c->spoke_from) {
                if (!c->drawn_from)
                    spoke(c, px, py, &c->drawn_from);
                return;
            }
            endpoint = 1;
        } else if (count > c->from) {
            if (count == c->to) {
                if (c->spoke_to) {
                    if (!c->drawn_to)
                        spoke(c, px, py, &c->drawn_to);
                    return;
                }
                endpoint = 1;
            } else {
                between = count < c->to;
            }
        }
        if (!endpoint && between == c->outside)
            return;
    }
    gfx_plot(px, py, c->color, OP_SET);
}

/* The eight points of a circle point (x, y), with y the count in its octant:
   the octants run counterclockwise from the right, the screen's y down. */
static void octants(Circle *c, long x, long y)
{
    long n = c->points;

    plot(c, x, -y, y);
    plot(c, y, -x, 2 * n - y);
    plot(c, -y, -x, 2 * n + y);
    plot(c, -x, -y, 4 * n - y);
    plot(c, -x, y, 4 * n + y);
    plot(c, -y, x, 6 * n - y);
    plot(c, y, x, 6 * n + y);
    plot(c, x, y, 8 * n - y);
}

static void rim(Circle *c)
{
    long x = c->radius, y = 0, sum = 1 - c->radius;

    for (;;) {
        octants(c, x, y);
        if (y >= x)
            break;
        if (sum >= 0) {
            sum += 2 - 2 * x;
            x--;
        }
        sum += 2 * y + 3;
        y++;
    }
}

/* An angle as the circle takes it: not past a turn. */
static double turn(double angle, int *spoked)
{
    *spoked = angle < 0;
    if (angle < 0)
        angle = -angle;
    if (angle > 2 * pi)
        qb_error(BE_ILLFUN);
    return angle;
}

/* B$CIRC: the radius and the colour (-1 for the foreground). */
void B_CIRC(float radius, int color)
{
    Circle c;
    double ratio = has_aspect ? aspect_ratio : gfx_current->aspect;
    double from = 0, to = 2 * pi;
    int spoke_from = 0, spoke_to = 0, arc = has_start || has_end, has_to = has_end;

    c.color = gfx_color(color);
    if (radius < 0 || ratio <= 0)
        qb_error(BE_ILLFUN);
    c.radius = (long)(radius + 0.5);
    c.points = (long)(c.radius * 0.7071067811865476 + 0.5);
    c.scale_x = ratio >= 1;
    c.scale = (long)((c.scale_x ? FIXED / ratio : FIXED * ratio) + 0.5);
    c.cx = gfx_x1;
    c.cy = gfx_y1;
    c.arc = arc;
    if (has_start)
        from = turn(start_angle, &spoke_from);
    if (has_end)
        to = turn(end_angle, &spoke_to);
    has_start = has_end = has_aspect = 0;
    c.spoke_from = (byte)spoke_from;
    c.spoke_to = (byte)spoke_to;
    c.drawn_from = c.drawn_to = 0;
    c.outside = 0;
    if (arc) {
        /* no end angle is an end past every count */
        unsigned long first = count_of(&c, from);
        unsigned long last = has_to ? count_of(&c, to) : 0xFFFFUL;

        if (last < first) {
            unsigned long swap = first;

            first = last;
            last = swap;
            c.outside = 1;
            c.spoke_from = (byte)spoke_to;     /* the ends change places */
            c.spoke_to = (byte)spoke_from;
        }
        if (first == last && (c.spoke_from || c.spoke_to))
            c.spoke_from = c.spoke_to = 1;
        c.from = first;
        c.to = last;
    }
    rim(&c);
}
#pragma aux B_CSTT "B$CSTT"
#pragma aux B_CSTO "B$CSTO"
#pragma aux B_CASP "B$CASP"
#pragma aux B_CIRC "B$CIRC"
