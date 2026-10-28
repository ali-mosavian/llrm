/* The speaker, in virtual time: BEEP, SOUND and PLAY run on the headless device, whose clock is a number, and the
   tone log they leave is rendered to samples by the sdl platform's own renderer (dev/sdl/tone.c). No wall clock, no
   audio device. Writes the samples to argv[1] as 16-bit mono at SDL_AUDIO_RATE, and one line a scenario to stdout:
   `name first_hundredth last_hundredth`. tests/sound.rs analyses them. */
#include <stdio.h>
#include <string.h>

#include "headless.h"
#include "nhstutil.h"
#include "qbsdl.h"
#include "rtinit.h"

extern void B_BEEP(void);
extern void B_SPLY(SD *commands);
extern void B_SOND(short hertz, float duration);

enum { GAP = 50 }; /* silence between scenarios, in hundredths */

static FILE *samples;
static long played; /* hundredths written */
static SdlTone tone;

static SD *string_of(const char *text)
{
    char *data;
    SD *sd = str_tmp((uword)strlen(text), &data);

    memcpy(data, text, strlen(text));
    return sd;
}

/* The log as samples from hundredth `begin` to `until`: each tone from the hundredth it was set (counted from `begin`)
   to the next change. */
static void render(long begin, long until)
{
    static int16_t block[SDL_AUDIO_RATE / 100];
    size_t at;
    long hundredth;

    for (hundredth = begin; hundredth < until; hundredth++) {
        unsigned hertz = 0;

        for (at = 0; at < headless_tone_count() && headless_tone(at).clock <= hundredth - begin; at++)
            hertz = headless_tone(at).hertz;
        sdl_audio_render(&tone, block, SDL_AUDIO_RATE / 100, hertz);
        fwrite(block, sizeof block[0], SDL_AUDIO_RATE / 100, samples);
        played++;
    }
}

/* Runs the speaker for `hundredths`, a hundredth at a time so the notes start when their ticks come. */
static void run(long hundredths)
{
    while (hundredths-- > 0)
        headless_run(1);
}

static void scenario(const char *name, long length)
{
    long begin = played;

    render(begin, begin + length + GAP);
    printf("%s %ld %ld\n", name, begin, begin + length);
    headless_reset();
    headless_clock_set(0, 0);
}

int main(int argc, char **argv)
{
    (void)argc;
    samples = fopen(argv[1], "wb");
    if (!samples)
        return 1;
    qb_start();

    headless_clock_set(0, 1); /* BEEP waits for its quarter second by reading the clock: each read is a hundredth */
    B_BEEP();
    scenario("beep", 25);

    headless_clock_set(0, 0);
    B_SOND(440, 18);
    run(120);
    scenario("sound_440_18", 120);

    B_SOND(880, 9);
    run(80);
    scenario("sound_880_9", 80);

    B_SOND(2000, 10);
    run(80);
    scenario("sound_2000_10", 80);

    B_SPLY(string_of("MB T120 O3 CDEFG"));
    run(300);
    scenario("play_cdefg", 300);

    fclose(samples);
    return 0;
}
