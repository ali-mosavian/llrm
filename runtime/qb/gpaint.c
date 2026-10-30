/* PAINT: fills the area round a point, up to a border colour (QB rt/paint.asm
   B$PAIN).  Spans a row at a time, each span painted whole and left to look at
   the row beyond it.  The spans wait where the target keeps a fill's queue
   (platform.h): in all the free string space, as QB's queue does, so a fill
   runs out of memory only where QB's would; or, where memory is flat, in a
   block of its own, so what the strings and the heap hold does not decide how
   much of a picture a fill can cover. */
#include "gfx.h"
#include "gfxdev.h"
#include "nhstutil.h"

/* A span of the fill that has been painted, whose neighbouring row `y + step`
 * is still to be looked at over `left` to `right`. */
typedef struct Span {
    short left, right, y, step;
} Span;

static Span *spans;
static unsigned span_count, span_room;

static void push(int left, int right, int y, int step)
{
    if (span_count == span_room)
        qb_error(BE_MEMORY);
    spans[span_count].left = left;
    spans[span_count].right = right;
    spans[span_count].y = y;
    spans[span_count].step = step;
    span_count++;
}

#pragma aux B_PAIN "B$PAIN"
#pragma aux B_PNTC "B$PNTC"

/* B$PAIN: the fill colour and the border colour (-1 for the foreground, and for
   the border the fill colour). */
void B_PAIN(short fill, short border)
{
    byte paint = gfx_color(fill);
    byte edge = border == -1 ? paint : gfx_color(border);
    unsigned long bytes;
    GdFill painter;
    int c = gfx_pixel(gfx_x1, gfx_y1), left, right, last;

    span_count = 0;
    if (c < 0 || c == edge || c == paint)
        return;
    spans = (Span *)qb_paint_queue_open(&bytes);
    span_room = (unsigned)(bytes / sizeof(Span));
    gd_fill_select(&painter, paint, OP_SET);
    last = gfx_view_right;
    /* the first run is the one through the point, which is neither border nor
     * fill */
    gd_run(gfx_x1, gfx_x1, (unsigned)gfx_y1, edge, paint, last, &left, &right);
    painter.box(&painter, (unsigned)left, (unsigned)gfx_y1, (unsigned)(right - left + 1), 1);
    push(left, right, gfx_y1, -1);
    push(left, right, gfx_y1, 1);
    while (span_count) {
        Span span = spans[--span_count];
        int y = span.y + span.step, x = span.left;

        if (y < gfx_view_top || y > gfx_view_bottom)
            continue;
        /* each pixel here that is neither border nor fill is in a run the fill
           has not reached; the run is painted whole and left to look at the
           rows beyond it, in the direction away from the one it was found from,
           and over what it reaches past its parent's span, back */
        while (x <= span.right &&
               gd_run(x, span.right, (unsigned)y, edge, paint, last, &left, &right)) {
            painter.box(&painter, (unsigned)left, (unsigned)y, (unsigned)(right - left + 1), 1);
            push(left, right, y, span.step);
            /* the run can reach past the span it was found under, and there the
             * row it was found from may hold more of the area */
            if (left < span.left)
                push(left, span.left - 1, y, -span.step);
            if (right > span.right)
                push(span.right + 1, right, y, -span.step);
            x = right + 1;
        }
    }
    qb_paint_queue_close();
}


/* The tile of PAINT's string (QB rt/paint.asm FGSTRINI, llagrp.asm B$TileMod):
   one byte a row where the mode has one plane, a row of 8 pixels (4 in the 2
   bit mode, and in the 8 bit one the byte is the colour); where it has four,
   four bytes a row, one a plane. */
static const byte *tile;
static unsigned tile_length, tile_height;

static byte tile_color(int x, int y)
{
    const GfxMode *mode = gfx_current;
    unsigned row = (unsigned)y % tile_height;

    if (mode->planes == 1) {
        byte b = tile[row];

        if (mode->bits == 8)
            return b;
        if (mode->bits == 2)
            return (byte)((b >> (6 - 2 * (x & 3))) & 3);
        return (byte)((b >> (7 - (x & 7))) & 1);
    } else {
        byte color = 0;
        unsigned plane;

        for (plane = 0; plane < 4; plane++) {
            unsigned at = row * 4 + plane;
            byte b = at < tile_length ? tile[at] : 0;

            color |= (byte)(((b >> (7 - (x & 7))) & 1) << plane);
        }
        return color;
    }
}

/* B$PNTC: PAINT with a tile string, up to the border colour (-1 for the
   foreground).  The area is the same one B$PAIN reaches; the pixels the fill
   has taken are kept in a bitmap, as the tile may hold the colour it looks for.
   The background tile (-1, none) is not supported. */
void B_PNTC(SD *pattern, short border, short background)
{
    byte edge = gfx_color(border);
    unsigned width = gfx_current->width, height = gfx_current->height;
    unsigned long bytes, bitmap = ((unsigned long)width * height + 7) / 8;
    byte *seen;
    int left, right, c, x, y;

    (void)background;
    if (pattern->len == 0 || pattern->len > 255)
        qb_error(BE_ILLFUN);
    tile = (const byte *)pattern->ptr;
    tile_length = pattern->len;
    tile_height = (tile_length - 1) / gfx_current->planes + 1;
    if (tile_height > 64)
        qb_error(BE_ILLFUN);
    c = gfx_pixel(gfx_x1, gfx_y1);
    span_count = 0;
    if (c < 0 || c == edge) {
        str_tmp_free(pattern);
        return;
    }
    seen = (byte *)qb_paint_queue_open(&bytes);
    if (bytes <= bitmap)
        qb_error(BE_MEMORY);
    for (x = 0; (unsigned long)x < bitmap; x++)
        seen[x] = 0;
    spans = (Span *)(seen + bitmap);
    span_room = (unsigned)((bytes - bitmap) / sizeof(Span));
#define SEEN(px, py) (seen[((unsigned long)(py) * width + (px)) >> 3] & (1 << (((py) * width + (px)) & 7)))
#define OPEN(px, py) ((int)gfx_pixel(px, py) != edge && !SEEN(px, py))
#define TAKE(px, py) \
    (seen[((unsigned long)(py) * width + (px)) >> 3] |= (byte)(1 << (((py) * width + (px)) & 7)), \
     gfx_plot(px, py, tile_color(px, py), OP_SET))
    y = gfx_y1;
    left = right = gfx_x1;
    while (left > 0 && OPEN(left - 1, y))
        left--;
    while (right < (int)width - 1 && OPEN(right + 1, y))
        right++;
    for (x = left; x <= right; x++)
        TAKE(x, y);
    push(left, right, y, -1);
    push(left, right, y, 1);
    while (span_count) {
        Span span = spans[--span_count];

        y = span.y + span.step;
        if (y < 0 || y >= (int)height)
            continue;
        x = span.left;
        while (x <= span.right) {
            if (!OPEN(x, y)) {
                x++;
                continue;
            }
            left = right = x;
            while (left > 0 && OPEN(left - 1, y))
                left--;
            while (right < (int)width - 1 && OPEN(right + 1, y))
                right++;
            for (c = left; c <= right; c++)
                TAKE(c, y);
            push(left, right, y, span.step);
            if (left < span.left)
                push(left, span.left - 1, y, -span.step);
            if (right > span.right)
                push(span.right + 1, right, y, -span.step);
            x = right + 1;
        }
    }
    qb_paint_queue_close();
    str_tmp_free(pattern);
}
