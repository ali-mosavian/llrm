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
    long from, to;          /* counts round the circle */
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
static long count_of(const Circle *c, double angle)
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
    return (long)(along + base + 0.5);
}

static int wanted(const Circle *c, long count)
{
    if (!c->arc)
        return 1;
    if (c->from <= c->to)
        return count >= c->from && count <= c->to;
    return count >= c->from || count <= c->to;
}

static void plot(const Circle *c, long x, long y, long count)
{
    if (wanted(c, count))
        gfx_plot(c->cx + (int)(c->scale_x ? scaled(c, x) : x),
                 c->cy + (int)(c->scale_x ? y : scaled(c, y)), c->color,
                 OP_SET);
}

/* The eight points of a circle point (x, y), with y the count in its octant:
   the octants run counterclockwise from the right, the screen's y down. */
static void octants(const Circle *c, long x, long y)
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

static void rim(const Circle *c)
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

/* A line from the centre to the arc's end at `count`. */
static void spoke(const Circle *c, double angle)
{
    double sine, cosine;
    long dx, dy;

    dev_sincos(&angle, &sine, &cosine);
    dx = (long)(c->radius * cosine + (cosine < 0 ? -0.5 : 0.5));
    dy = (long)(c->radius * sine + (sine < 0 ? -0.5 : 0.5));
    gfx_x1 = c->cx;
    gfx_y1 = c->cy;
    gfx_x2 = c->cx + (int)(c->scale_x ? scaled(c, dx) : dx);
    gfx_y2 = c->cy - (int)(c->scale_x ? dy : scaled(c, dy));
    gfx_line_between(c->color, -1, 0);
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
    int spoke_from = 0, spoke_to = 0, arc = has_start || has_end;

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
    if (arc) {
        c.from = count_of(&c, from);
        c.to = count_of(&c, to);
    }
    rim(&c);
    if (spoke_from)
        spoke(&c, from);
    if (spoke_to)
        spoke(&c, to);
}
#pragma aux B_CSTT "B$CSTT"
#pragma aux B_CSTO "B$CSTO"
#pragma aux B_CASP "B$CASP"
#pragma aux B_CIRC "B$CIRC"
