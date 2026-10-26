/* The speaker's samples, apart from SDL so that a test can render a tone log without a device. */
#include "qbsdl.h"

enum { RATE = SDL_AUDIO_RATE, AMPLITUDE = 6000, RAMP = 120 };

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
