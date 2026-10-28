/* DRAW (QB rt/draw.asm B$DRAW): the graphics macro language.  U D L R E F G H
   move by a number of units (of the scale, in quarters) from the last point,
   M moves to a point or by an offset, B moves without drawing, N returns to
   the start, C sets the colour, S the scale, A and TA the angle, P paints. */
#include "error.h"
#include "gfx.h"
#include "nhstutil.h"
#include "qb.h"
#include "varptr.h"

#pragma aux B_PAIN "B$PAIN"
#pragma aux B_DRAW "B$DRAW"

void B_PAIN(short fill, short border);

enum { FRACTION = 256, HALF = 128 };

enum { NO_DRAW = 1, NO_MOVE = 2 };

typedef struct Drawing {
    Macro m;
    int scale;  /* quarters; 0 for none, as QB's initial */
    int angle;  /* degrees */
    byte flags; /* B and N, for the next move */
    int fx, fy; /* the fraction of a pixel the position is off by, 1/256 */
} Drawing;

static void illegal(void)
{
    qb_error(BE_ILLFUN);
}

/* A number with its signs, or `fallback` where there is none.  Up to five
   digits, wrapping as a 16-bit integer does; more is an error. */
static long number(Drawing *d, long fallback)
{
    unsigned value = 0;
    int sign = 1, digits = 0;

    for (;;) {
        while (d->m.at < d->m.end && *d->m.at == ' ')
            d->m.at++;
        if (d->m.at < d->m.end && (*d->m.at == '+' || *d->m.at == '-'))
            sign = *d->m.at++ == '-' ? -sign : sign;
        else
            break;
    }
    while (d->m.at < d->m.end && *d->m.at >= '0' && *d->m.at <= '9') {
        value = value * 10 + (unsigned)(*d->m.at++ - '0');
        if (++digits > 5)
            illegal();
    }
    if (!digits) {
        long value;

        if (macro_equals(&d->m, &value))
            return sign * (long)(short)value;
    }
    return digits ? sign * (long)(short)value : fallback;
}

/* sine and cosine of an angle in degrees: exact at the right angles */
static void sine_cosine(int degrees, double *sine, double *cosine)
{
    double x, term, sum, squared;
    int k;

    degrees %= 360;
    if (degrees < 0)
        degrees += 360;
    if (degrees % 90 == 0) {
        static const int s[4] = {0, 1, 0, -1}, c[4] = {1, 0, -1, 0};

        *sine = s[degrees / 90];
        *cosine = c[degrees / 90];
        return;
    }
    x = degrees * 0.017453292519943295;
    squared = x * x;
    term = sum = x;
    for (k = 1; k < 12; k++) {
        term = -term * squared / ((2 * k) * (2 * k + 1));
        sum += term;
    }
    *sine = sum;
    term = sum = 1;
    for (k = 1; k < 12; k++) {
        term = -term * squared / ((2 * k - 1) * (2 * k));
        sum += term;
    }
    *cosine = sum;
}

/* A distance in units as pixels of the scale, truncated toward zero. */
static long scaled(const Drawing *d, long units)
{
    return d->scale ? units * d->scale / 4 : units;
}

static void move_to(Drawing *d, int x, int y)
{
    if (!(d->flags & NO_DRAW)) {
        gfx_x1 = gfx_last_x;
        gfx_y1 = gfx_last_y;
        gfx_x2 = x;
        gfx_y2 = y;
        gfx_line_between(-1, -1, 0);
    }
    if (!(d->flags & NO_MOVE)) {
        gfx_last_x = x;
        gfx_last_y = y;
    }
    d->flags = 0;
}

/* A move by (dx, dy) units: scaled, turned by the angle (the pixels not being
   square, a turn stretches by their aspect), and made from the fraction of a
   pixel the last move left. */
static void move_by(Drawing *d, long dx, long dy)
{
    long x = scaled(d, dx), y = scaled(d, dy);

    if (d->angle % 360 != 0) {
        double sine, cosine, aspect = gfx_current->aspect;
        long fixed_x, fixed_y;

        sine_cosine(d->angle, &sine, &cosine);
        fixed_x = (long)((x * cosine + y * sine / aspect) * FRACTION) + d->fx;
        fixed_y = (long)((x * -sine * aspect + y * cosine) * FRACTION) + d->fy;
        x = fixed_x >> 8;
        y = fixed_y >> 8;
        d->fx = (int)(fixed_x & 0xFF);
        d->fy = (int)(fixed_y & 0xFF);
    }
    move_to(d, gfx_last_x + (int)x, gfx_last_y + (int)y);
}

static void absolute(Drawing *d, long x, long y)
{
    d->fx = d->fy = HALF;
    move_to(d, (int)x + gfx_origin_x, (int)y + gfx_origin_y);
}

/* M: to a point, or by an offset if the first number has a sign. */
static void move(Drawing *d)
{
    int relative;
    long x, y;

    while (d->m.at < d->m.end && *d->m.at == ' ')
        d->m.at++;
    relative = d->m.at < d->m.end && (*d->m.at == '+' || *d->m.at == '-');
    x = number(d, 0);
    while (d->m.at < d->m.end && *d->m.at == ' ')
        d->m.at++;
    if (d->m.at >= d->m.end || *d->m.at++ != ',')
        illegal();
    y = number(d, 0);
    if (relative)
        move_by(d, x, y);
    else
        absolute(d, x, y);
}

/* B$DRAW: DRAW with the string of commands. */
/* The commands of the string `d` is at, to its end. */
static void execute(Drawing *d)
{
    char command;
    long n;

    while ((command = macro_next(&d->m)) != 0) {
        switch (command) {
        case 'U':
            move_by(d, 0, -number(d, 1));
            break;
        case 'D':
            move_by(d, 0, number(d, 1));
            break;
        case 'L':
            move_by(d, -number(d, 1), 0);
            break;
        case 'R':
            move_by(d, number(d, 1), 0);
            break;
        case 'E':
            n = number(d, 1);
            move_by(d, n, -n);
            break;
        case 'F':
            n = number(d, 1);
            move_by(d, n, n);
            break;
        case 'G':
            n = number(d, 1);
            move_by(d, -n, n);
            break;
        case 'H':
            n = number(d, 1);
            move_by(d, -n, -n);
            break;
        case 'M':
            move(d);
            break;
        case 'B':
            d->flags |= NO_DRAW;
            break;
        case 'N':
            d->flags |= NO_MOVE;
            break;
        case 'C':
            n = number(d, -1);
            if (n < 0)
                illegal();
            /* the attribute is the low byte of the number */
            if ((n & 0xFF) >= (long)gfx_current->colors)
                illegal();
            gfx_foreground = (byte)(n & 0xFF);
            break;
        case 'S':
            n = number(d, -1);
            if (n < 1 || n > 255)
                illegal();
            d->scale = (int)n;
            break;
        case 'A':
            n = number(d, -1);
            if (n < 0 || n > 3)
                illegal();
            d->angle = (int)n * 90;
            break;
        case 'T':
            if (macro_next(&d->m) != 'A')
                illegal();
            n = number(d, 0);
            if (n < 0)
                n += 360;
            if (n > 360)
                illegal();
            d->angle = (int)n;
            break;
        case 'P': {
            long border;

            n = number(d, -1);
            if (n < 0 || d->m.at >= d->m.end || *d->m.at++ != ',')
                illegal();
            border = number(d, -1);
            if (border < 0)
                illegal();
            gfx_x1 = gfx_last_x;
            gfx_y1 = gfx_last_y;
            B_PAIN((short)n, (short)border);
            break;
        }
        case 'X': {
            /* the commands of a string variable, then on with these */
            Macro outer;

            macro_enter(&d->m, &outer);
            execute(d);
            macro_leave(&d->m, &outer);
            break;
        }
        default:
            illegal();
        }
    }
}

/* B$DRAW: DRAW with the string of commands. */
void B_DRAW(SD *commands)
{
    /* the scale, the angle and the fractions stay as they were from one DRAW
       to the next */
    static Drawing d = {{0, 0, 0}, 0, 0, 0, HALF, HALF};

    if (!gfx_current)
        illegal();
    macro_begin(&d.m, commands);
    d.flags = 0;
    execute(&d);
    str_tmp_free(commands);
}
