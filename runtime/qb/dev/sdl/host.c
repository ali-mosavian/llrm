/* The host's main(): SDL on the main thread (macOS needs it there for the window), the QB program on a worker. The
   program is qb_host_program(); when it ends, or the window is closed and it notices, the host stops SDL from this
   explicit path and exits with its code. QB_SDL_HOLD=1 keeps the window until it is closed or a key is pressed. */
#define _POSIX_C_SOURCE 200809L
#include <SDL.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <unistd.h>

#include "dev.h"
#include "llrm_os.h"
#include "qbsdl.h"
#include "tick.h"
#include "video.h"

enum { FRAME_MS = 16, CLOSE_GRACE_MS = 1500 };

static pthread_t worker;
static atomic_int finished;
static int exit_code, argument_count;
static char **arguments;

/* Called by llrm_os_exit on the worker: the program is over. */
static void ended(int code)
{
    exit_code = code;
    atomic_store(&finished, 1);
    pthread_exit(0);
}

static void *run(void *unused)
{
    (void)unused;
    llrm_os_exit((unsigned char)qb_host_program(argument_count, arguments));
    return 0;
}

static long long now_ms(void)
{
    struct timespec now;

    clock_gettime(CLOCK_MONOTONIC, &now);
    return (long long)now.tv_sec * 1000 + now.tv_nsec / 1000000;
}

static void frame(void)
{
    unsigned upto;

    sdl_input_pump();
    upto = sdl_sync_begin();
    sdl_present_frame();
    sdl_sync_done(upto);
}

int main(int argc, char **argv)
{
    long long closed_at = 0;

    argument_count = argc;
    arguments = argv;
    llrm_os_set_arguments(argc, argv);
    if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_EVENTS) != 0 || sdl_present_open() != 0) {
        fprintf(stderr, "SDL: %s\n", SDL_GetError());
        return 1;
    }
    if (SDL_InitSubSystem(SDL_INIT_AUDIO) == 0)
        sdl_audio_open();
    sdl_input_open();
    host_video_reset();
    host_tick_set_thread(1);
    llrm_os_set_exit_hook(ended);
    if (pthread_create(&worker, 0, run, 0) != 0) {
        fprintf(stderr, "cannot start the program's thread\n");
        return 1;
    }
    while (!atomic_load(&finished)) {
        frame();
        if (sdl_interrupted()) {
            if (!closed_at)
                closed_at = now_ms();
            else if (now_ms() - closed_at > CLOSE_GRACE_MS)
                _exit(1); /* the program never looked at a device */
        }
        sdl_sync_wait_request(FRAME_MS);
    }
    pthread_join(worker, 0);
    frame();
    if (getenv("QB_SDL_HOLD") && !sdl_interrupted()) {
        while (!sdl_interrupted() && !sdl_key_waiting()) {
            frame();
            SDL_Delay(FRAME_MS);
        }
    }
    sdl_audio_close();
    sdl_present_close();
    SDL_Quit();
    return exit_code;
}
