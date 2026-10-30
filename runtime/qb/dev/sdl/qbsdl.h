/* The sdl device platform's modules (docs/working/qb-host.md): the program runs on a worker thread, SDL on the main
   thread. The worker never calls SDL; it writes the framebuffer (host/video.h), the key queue's reader end, the
   tone, and asks for a flush. This header is what the modules say to each other. */
#ifndef QB_SDL_H
#define QB_SDL_H

#include <stdint.h>

/* present.c: the window, the renderer and one streaming texture. */
int sdl_present_open(void);
void sdl_present_close(void);
/* Draws the lines the program has touched since the last call; whether a frame went to the screen. */
int sdl_present_frame(void);
/* The last frame as ARGB, `width` by `height`, for a test; null before the first. */
const uint32_t *sdl_present_argb(unsigned *width, unsigned *height);

/* input.c: SDL's events become keys (the main thread), and the keys are read by the worker. */
void sdl_input_open(void);
/* Handles the events waiting; whether the window was asked to close. */
void sdl_input_pump(void);
int sdl_key_waiting(void);
/* The next key as a byte, or 0x100 | scan for an extended one; -1 if there is none. */
int sdl_key_take(void);
int sdl_key_peek_scan(void);
void sdl_key_push(unsigned ascii, unsigned scan); /* the main thread, or a test */
int sdl_interrupted(void);
void sdl_interrupt(void);

/* audio.c: a square wave of the tone the program last set, from SDL's audio callback. */
void sdl_audio_open(void);
void sdl_audio_close(void);
void sdl_audio_tone(unsigned hertz);
/* The tone the callback is making, for a test. */
unsigned sdl_audio_current(void);
/* The samples of the tone `hertz` (0 for silence): what the audio callback fills. */
enum { SDL_AUDIO_RATE = 44100 }; /* samples a second, mono, 16 bits */
typedef struct SdlTone {
    unsigned phase;
    int gain; /* 0 to the fade length */
} SdlTone;
void sdl_audio_render(SdlTone *tone, int16_t *out, int count, unsigned hertz);

typedef struct {
    int x, y, w, h;
} SdlRect;

SdlRect sdl_fit(int window_width, int window_height, int frame_width);

/* sync.c: the worker waits for the next frame to reach the screen. */
void sdl_sync_request(void);                  /* the worker: wake the SDL thread and wait for a frame, up to one interval */
void sdl_sync_wait_request(int timeout_ms);   /* the SDL thread: sleep until a request or the interval */
unsigned sdl_sync_requests(void);             /* the requests made so far (a test) */
unsigned sdl_sync_begin(void);                /* the SDL thread: the requests this frame will answer */
void sdl_sync_done(unsigned upto);            /* the SDL thread: the frame has been presented */

/* host.c: the program's entry, which the program (or a test) supplies. */
extern int qb_host_program(int argc, char **argv);

#endif
