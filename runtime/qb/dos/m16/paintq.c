/* A fill's queue (gpaint.c) in all the free string space, as QB's is, so a fill
 * runs out of memory where QB's would. */
#include "gfx.h"
#include "nheap.h"
#include "nhstutil.h"

static SD *held;

void *qb_paint_queue_open(unsigned long *bytes)
{
    char *space;
    unsigned room;

    /* an open file has taken the string space for the heap, free room and all
       */
    lh_give_free_to_strings();
    room = str_free_bytes();

    /* The queue is one string, whose length is a word, whatever the free room.
     */
    if (room > SD_MAX_LENGTH)
        room = SD_MAX_LENGTH;
    held = str_tmp(room, &space);
    *bytes = room;
    return space;
}

void qb_paint_queue_close(void)
{
    str_tmp_free(held);
}
