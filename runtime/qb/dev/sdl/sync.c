/* The worker's flush points (docs/working/qb-host.md): draw-and-erase pairs between two frames show nothing unless
   the program waits for one. The wait is at most a frame, so a program that flushes in a loop runs at the display's
   rate, not slower. */
#define _POSIX_C_SOURCE 200809L
#include <pthread.h>
#include <stdatomic.h>
#include <time.h>

#include "qbsdl.h"

enum { FRAME_MS = 16, NANOS_PER_MS = 1000000, MS_PER_SECOND = 1000 };

static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t wake_main = PTHREAD_COND_INITIALIZER, frame_done = PTHREAD_COND_INITIALIZER;
static unsigned requested, presented;
static atomic_llong last_request_ms;

static long long now_ms(void)
{
    struct timespec now;

    clock_gettime(CLOCK_MONOTONIC, &now);
    return (long long)now.tv_sec * MS_PER_SECOND + now.tv_nsec / NANOS_PER_MS;
}

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
    last_request_ms = now_ms();
}

void sdl_sync_yield_if_due(void)
{
    if (now_ms() - last_request_ms >= FRAME_MS)
        sdl_sync_request();
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
