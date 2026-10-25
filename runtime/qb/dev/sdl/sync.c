/* The worker's flush points (docs/working/qb-host.md): a mode change and a wait for a key block until a frame is
   shown. Key and clock polls do not: the SDL thread presents every frame interval on its own, so an animation that polls
   in a loop (SparklePause, a Rest) is sampled at any phase, as a raster scan samples it, instead of always at the end of
   its cycle. */
#define _POSIX_C_SOURCE 200809L
#include <pthread.h>
#include <time.h>

#include "qbsdl.h"

enum { FRAME_MS = 16, NANOS_PER_MS = 1000000, MS_PER_SECOND = 1000 };

static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t wake_main = PTHREAD_COND_INITIALIZER, frame_done = PTHREAD_COND_INITIALIZER;
static unsigned requested, presented;

static void deadline_in(struct timespec *at, int ms)
{
    clock_gettime(CLOCK_REALTIME, at);
    at->tv_nsec += (long)ms * NANOS_PER_MS;
    at->tv_sec += at->tv_nsec / (MS_PER_SECOND * (long)NANOS_PER_MS);
    at->tv_nsec %= MS_PER_SECOND * (long)NANOS_PER_MS;
}

void sdl_sync_request(void)
{
    struct timespec until;
    unsigned mine;

    pthread_mutex_lock(&lock);
    mine = ++requested;
    pthread_cond_signal(&wake_main);
    deadline_in(&until, FRAME_MS * 2);
    while ((int)(presented - mine) < 0 && pthread_cond_timedwait(&frame_done, &lock, &until) == 0)
        ;
    pthread_mutex_unlock(&lock);
}

void sdl_sync_wait_request(int timeout_ms)
{
    struct timespec until;

    pthread_mutex_lock(&lock);
    if (requested == presented) {
        deadline_in(&until, timeout_ms);
        pthread_cond_timedwait(&wake_main, &lock, &until);
    }
    pthread_mutex_unlock(&lock);
}

/* The requests made so far, for a test of what asks for a frame. */
unsigned sdl_sync_requests(void)
{
    unsigned count;

    pthread_mutex_lock(&lock);
    count = requested;
    pthread_mutex_unlock(&lock);
    return count;
}

unsigned sdl_sync_begin(void)
{
    unsigned upto;

    pthread_mutex_lock(&lock);
    upto = requested;
    pthread_mutex_unlock(&lock);
    return upto;
}

void sdl_sync_done(unsigned upto)
{
    pthread_mutex_lock(&lock);
    presented = upto;
    pthread_cond_broadcast(&frame_done);
    pthread_mutex_unlock(&lock);
}
