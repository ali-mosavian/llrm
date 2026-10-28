/* The dirty-line protocol (runtime/qb/dev/host/video.h): the writer fills a line, publishes its pass number with a
   release store and sets the line's flag; the presenter takes the flag with an acquire exchange, loads the pass number
   with acquire and must see the whole line of that pass or a later one. A byte of an earlier pass is a torn line (a
   weak memory order lets that happen on arm64 where x86's does not). Pixel bytes are relaxed atomics: the race on
   them is by design. */
#include <pthread.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <time.h>
#include "video.h"

enum { LINES = 8, PASSES = 100000 };

static _Atomic unsigned published[LINES], consumed[LINES];
static _Atomic int done, stop;
static unsigned torn;

static void *writer(void *unused)
{
    uint8_t *pixels = (uint8_t *)host_pixels();
    unsigned width = host_width();

    (void)unused;
    for (unsigned pass = 1; pass <= PASSES; pass++) {
        for (unsigned y = 0; y < LINES; y++) {
            /* a byte wraps at 256 passes: the presenter may not fall further behind than that tells apart */
            while (pass - atomic_load_explicit(&consumed[y], memory_order_relaxed) > 64 && !atomic_load(&stop))
                ;
            for (unsigned x = 0; x < width; x++)
                __atomic_store_n(&pixels[y * width + x], (uint8_t)pass, __ATOMIC_RELAXED);
            atomic_store_explicit(&published[y], pass, memory_order_release);
            atomic_store_explicit(&host_dirty[y], 1, memory_order_release);
        }
    }
    atomic_store(&done, 1);
    return 0;
}

int main(void)
{
    pthread_t thread;
    const uint8_t *pixels;
    unsigned width, seen = 0;

    host_video_reset();
    pixels = host_pixels();
    width = host_width();
    pthread_create(&thread, 0, writer, 0);
    while (!atomic_load(&done)) {
        for (unsigned y = 0; y < LINES; y++) {
            if (!atomic_exchange_explicit(&host_dirty[y], 0, memory_order_acquire))
                continue;
            unsigned low = atomic_load_explicit(&published[y], memory_order_acquire);

            seen++;
            /* the flag was set after `published` was stored and the line written, so the line shows at least the
               pass `low` had when this exchange saw it, modulo the byte's wrap */
            for (unsigned x = 0; x < width; x++) {
                unsigned byte = __atomic_load_n(&pixels[y * width + x], __ATOMIC_RELAXED);
                unsigned back = (low - byte) & 0xff;

                if (back != 0 && back < 0x80)
                    torn++;
            }
            atomic_store_explicit(&consumed[y], low, memory_order_relaxed);
        }
    }
    pthread_join(thread, 0);
    printf(torn ? "torn %u of %u lines\n" : "ok\n", torn ? torn : seen, seen);
    return torn != 0;
}
