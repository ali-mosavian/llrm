/* The speaker: a square wave of the last tone the program set, made in SDL's audio callback. A tone that starts or stops
   ramps over a few milliseconds so it does not click. If there is no audio device the program runs, silent. */
#include <SDL.h>
#include <stdatomic.h>

#include "qbsdl.h"

enum { RATE = 44100, SAMPLES = 512, AMPLITUDE = 6000, RAMP = 120 };

static SDL_AudioDeviceID device;
static atomic_uint wanted;
static atomic_uint making;

unsigned sdl_audio_current(void)
{
    return atomic_load_explicit(&making, memory_order_relaxed);
}

void sdl_audio_tone(unsigned hertz)
{
    atomic_store_explicit(&wanted, hertz, memory_order_relaxed);
}

/* The tone as samples: a hard square wave at full amplitude, as the PC speaker's, faded in and out over RAMP samples
   where the tone starts and stops so that it does not click. the `SdlTone` carries `phase` and `gain` on from call to call. */
void sdl_audio_render(SdlTone *tone, int16_t *out, int count, unsigned hertz)
{
    unsigned phase = tone->phase;
    int gain = tone->gain, at;

    for (at = 0; at < count; at++) {
        int wave = 0;

        if (hertz) {
            phase += hertz;
            if (phase >= RATE)
                phase -= RATE;
            wave = phase < RATE / 2 ? AMPLITUDE : -AMPLITUDE;
        }
        gain += hertz ? (gain < RAMP) : -(gain > 0);
        out[at] = (int16_t)(wave * gain / RAMP);
    }
    tone->phase = phase;
    tone->gain = gain;
}

static void fill(void *unused, Uint8 *stream, int bytes)
{
    unsigned hertz = atomic_load_explicit(&wanted, memory_order_relaxed);

    static SdlTone tone;

    (void)unused;
    atomic_store_explicit(&making, hertz, memory_order_relaxed);
    sdl_audio_render(&tone, (int16_t *)stream, bytes / (int)sizeof(int16_t), hertz);
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
