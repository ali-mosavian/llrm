/* PAINT: fills the area round a point, up to a border colour (QB rt/paint.asm
   B$PAIN).  Spans a row at a time, each span painted whole and left to look at the row beyond it.  The spans wait where the target keeps a fill's queue (platform.h): in all the free string space,
   as QB's queue does, so a fill runs out of memory only where QB's would; or, where memory is flat, in a block of
   its own, so what the strings and the heap hold does not decide how much of a picture a fill can cover. */
#include "gfx.h"
#include "gfxdev.h"

/* A span of the fill that has been painted, whose neighbouring row `y + step` is still to be looked at over `left` to `right`. */
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

/* The whole run through `x` of row `y` that is not the border, painted; the span is left to look at the rows beyond it, in
   the direction away from the one it was found from (the first one, from nowhere, both ways), and over what it reaches past
   its parent's span, back. */
static void paint_run(GdFill *painter, int x, int y, int edge, int *left, int *right)
{
    int l = gfx_search(x, 0, y, edge, edge, 1), r = gfx_search(x, (int)gfx_current->width - 1, y, edge, edge, 1);

    *left = l < 0 ? 0 : l + 1;
    *right = r < 0 ? (int)gfx_current->width - 1 : r - 1;
    painter->box(painter, (unsigned)*left, (unsigned)y, (unsigned)(*right - *left + 1), 1);
}

/* B$PAIN: the fill colour and the border colour (-1 for the foreground, and for
   the border the fill colour). */
void B_PAIN(short fill, short border)
{
    byte paint = gfx_color(fill);
    byte edge = border == -1 ? paint : gfx_color(border);
    unsigned long bytes;
    GdFill painter;
    int c = gfx_pixel(gfx_x1, gfx_y1), left, right;

    span_count = 0;
    if (c < 0 || c == edge || c == paint)
        return;
    spans = (Span *)qb_paint_queue_open(&bytes);
    span_room = (unsigned)(bytes / sizeof(Span));
    gd_fill_select(&painter, paint, OP_SET);
    paint_run(&painter, gfx_x1, gfx_y1, edge, &left, &right);
    push(left, right, gfx_y1, -1);
    push(left, right, gfx_y1, 1);
    while (span_count) {
        Span span = spans[--span_count];
        int y = span.y + span.step, x = span.left, open;

        if (y < 0 || y >= (int)gfx_current->height)
            continue;
        /* each pixel here that is neither border nor fill is in a run the fill has not reached */
        while (x <= span.right && (open = gfx_search(x, span.right, y, edge, paint, 0)) >= 0) {
            paint_run(&painter, open, y, edge, &left, &right);
            push(left, right, y, span.step);
            /* the run can reach past the span it was found under, and there the row it was found from may hold more of the area */
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
