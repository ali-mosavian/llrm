/* A fill's queue (gpaint.c) in a block of its own, kept for the next fill: memory is flat, so what the strings and the
   heap happen to hold does not limit how much of a picture a fill can cover.  A fill queues at most a seed for
   each run of each row: half a row's pixels, and a row for each of the screen's. */
#include "gfx.h"
#include "llrm_os.h"

enum { SEEDS_PER_PIXEL_DIVISOR = 2, SEED_BYTES = 4 };

static void *block;
static unsigned long block_bytes;

void *qb_paint_queue_open(unsigned long *bytes)
{
    unsigned long need = (unsigned long)gfx_current->width * gfx_current->height / SEEDS_PER_PIXEL_DIVISOR * SEED_BYTES;

    if (block_bytes < need) {
        void *more = (void *)llrm_os_more(need);

        if (more) {
            block = more;
            block_bytes = need;
        }
    }
    *bytes = block ? block_bytes : 0;
    return block;
}

void qb_paint_queue_close(void)
{
}
