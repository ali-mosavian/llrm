#define _POSIX_C_SOURCE 200809L
#include <pthread.h>
#include <stdatomic.h>
#include <time.h>

#include "dev.h"
#include "tick.h"

enum { TICK_NANOSECONDS = 54925400 }; /* 1 / 18.2065 s */

/* A weak no-op when the program has no music (play.c is not linked); not an undefined reference, which Mach-O's linker
   rejects. */
__attribute__((weak)) void music_tick(void) {}

static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static atomic_int started, use_thread, thread_running;
static pthread_t thread;

static void *ticking(void *unused);

static void thread_start(void)
{
    if (!thread_running) {
        thread_running = 1;
        pthread_create(&thread, 0, ticking, 0);
    }
}

static void thread_stop(void)
{
    if (thread_running) {
        thread_running = 0;
        pthread_join(thread, 0);
    }
}

void host_tick_set_thread(int yes)
{
    use_thread = yes;
    if (yes && started)
        thread_start();
    else if (!yes)
        thread_stop();
}

int host_tick_fire(void)
{
    int ran = 0;

    if (pthread_mutex_trylock(&lock) == 0) {
        if (started) {
            music_tick();
            ran = 1;
        }
        pthread_mutex_unlock(&lock);
    }
    return ran;
}

static void *ticking(void *unused)
{
    struct timespec wait = {0, TICK_NANOSECONDS};

    (void)unused;
    while (thread_running) {
        nanosleep(&wait, 0);
        pthread_mutex_lock(&lock);
        if (started)
            music_tick();
        pthread_mutex_unlock(&lock);
    }
    return 0;
}

void dev_tick_start(void)
{
    started = 1;
    if (use_thread)
        thread_start();
}

void dev_tick_stop(void)
{
    started = 0;
    thread_stop();
}

void dev_tick_hold(void)
{
    pthread_mutex_lock(&lock);
}

void dev_tick_release(void)
{
    pthread_mutex_unlock(&lock);
}
