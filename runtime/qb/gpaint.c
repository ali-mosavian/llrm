/* PAINT: fills the area round a point, up to a border colour (QB rt/paint.asm
   B$PAIN).  Spans a row at a time, each span painted whole and left to look at
   the row beyond it.  The spans wait where the target keeps a fill's queue
   (platform.h): in all the free string space, as QB's queue does, so a fill
   runs out of memory only where QB's would; or, where memory is flat, in a
   block of its own, so what the strings and the heap hold does not decide how
   much of a picture a fill can cover. */
#include "gfx.h"
#include "gfxdev.h"

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

#pragma aux B_PAIN "B$PAIN"
