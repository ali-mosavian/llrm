/* The speaker: a square wave of the last tone the program set, made in SDL's audio callback. A tone that starts or stops
   ramps over a few milliseconds so it does not click. If there is no audio device the program runs, silent. */
#include <SDL.h>
#include <stdatomic.h>

#include "sdl.h"

enum { RATE = 44100, SAMPLES = 512, AMPLITUDE = 6000, RAMP = 120 };

static SDL_AudioDeviceID device;
static atomic_uint wanted;
static unsigned making;

unsigned sdl_audio_current(void)
{
    return making;
}

void sdl_audio_tone(unsigned hertz)
{
    atomic_store_explicit(&wanted, hertz, memory_order_relaxed);
}

static void fill(void *unused, Uint8 *stream, int bytes)
{
    static unsigned phase;
    static int level;
    int16_t *out = (int16_t *)stream;
    int count = bytes / (int)sizeof *out, at;
    unsigned hertz = atomic_load_explicit(&wanted, memory_order_relaxed);

    (void)unused;
    making = hertz;
    for (at = 0; at < count; at++) {
        int target = 0;

        if (hertz) {
            phase += hertz;
            if (phase >= RATE)
                phase -= RATE;
            target = phase < RATE / 2 ? AMPLITUDE : -AMPLITUDE;
        }
        level += target > level ? (target - level > RAMP ? RAMP : target - level)
                                : -(level - target > RAMP ? RAMP : level - target);
        out[at] = (int16_t)level;
    }
}

void sdl_audio_open(void)
{
    SDL_AudioSpec want, have;

    SDL_zero(want);
    want.freq = RATE;
    want.format = AUDIO_S16SYS;
    want.channels = 1;
    want.samples = SAMPLES;
    want.callback = fill;
    device = SDL_OpenAudioDevice(0, 0, &want, &have, 0);
    if (device)
        SDL_PauseAudioDevice(device, 0);
}

void sdl_audio_close(void)
{
    if (device)
        SDL_CloseAudioDevice(device);
    device = 0;
}
