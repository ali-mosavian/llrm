/* The sdl device platform under SDL's dummy video and audio drivers: the program prints, draws, types and sounds on a
   worker thread; the SDL thread shows it. Checks that the frame on its way to the window is the framebuffer in the
   palette's colours, that keys made of SDL events reach INKEY$, that a tone reaches the audio callback, and that closing
   the window ends the program. Prints ok, or what differed. */
#include <SDL.h>
#include <stdio.h>
#include <string.h>

#include "console.h"
#include "dev.h"
#include "gfx.h"
#include "gfxdev.h"
#include "nhstutil.h"
#include "rtinit.h"
#include "qbsdl.h"
#include "video.h"

extern SD *B_INKY(void);
extern void B_N1I2(short x, short y);
extern void B_N2I2(short x, short y);
extern void B_LINE(short color, short style, short how);

static int failures;

#define CHECK(condition)                                                                   \
    do {                                                                                   \
        if (!(condition)) {                                                                \
            printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #condition);                     \
            failures++;                                                                    \
        }                                                                                  \
    } while (0)

/* The frame the SDL thread last drew against what the framebuffer and palette say it should be. */
static int frame_matches_once(void)
{
    uint8_t rgb[256][3];
    unsigned width, height, x, y, wrong = 0;
    const uint32_t *shown;

    sdl_sync_request();
    sdl_sync_request(); /* the first can answer a frame that began before the last write */
    shown = sdl_present_argb(&width, &height);
    if (!shown || width != host_width() || height != host_height())
        return 0;
    host_state_lock();
    host_colors(rgb);
    host_state_unlock();
    for (y = 0; y < height; y++)
        for (x = 0; x < width; x++) {
            const uint8_t *color = rgb[host_pixels()[y * width + x]];
            uint32_t want = 0xFF000000u | (uint32_t)color[0] << 16 | (uint32_t)color[1] << 8 | color[2];

            /* the text cursor is drawn over the cell at the bottom of its row */
            if (shown[y * width + x] != want && host_mode() != HOST_MODE_TEXT)
                wrong++;
            else if (shown[y * width + x] != want && y % HOST_CELL_HEIGHT < HOST_CELL_HEIGHT - 2)
                wrong++;
        }
    return wrong == 0;
}

/* A flush waits at most a frame, which a slow SDL thread (a sanitizer) overruns: ask again for a second before saying no. */
static int frame_matches(void)
{
    unsigned attempt;

    for (attempt = 0; attempt < 100; attempt++) {
        if (frame_matches_once())
            return 1;
        SDL_Delay(10);
    }
    return 0;
}

static void press(SDL_Keycode key, const char *typed)
{
    SDL_Event event;

    SDL_zero(event);
    if (typed) {
        event.type = SDL_TEXTINPUT;
        strcpy(event.text.text, typed);
    } else {
        event.type = SDL_KEYDOWN;
        event.key.keysym.sym = key;
    }
    SDL_PushEvent(&event);
}

/* The tone is a full-amplitude square wave centred on zero, at every pitch (a slow ramp once made it a triangle with an
   offset of about 3000 that shrank as the pitch rose). */
static unsigned asked;

static void check_square_wave(void)
{
    static const unsigned pitches[] = {440, 880, 2000};
    int16_t block[4410];
    unsigned at, n;
    SdlTone tone = {0, 0};

    for (at = 0; at < sizeof pitches / sizeof pitches[0]; at++) {
        long sum = 0;
        int low = 0, high = 0;

        sdl_audio_render(&tone, block, 4410, pitches[at]);
        sdl_audio_render(&tone, block, 4410, pitches[at]); /* past the fade-in */
        for (n = 0; n < 4410; n++) {
            sum += block[n];
            low = block[n] < low ? block[n] : low;
            high = block[n] > high ? block[n] : high;
        }
        CHECK(high >= 5900 && low <= -5900);
        CHECK(sum / 4410 > -300 && sum / 4410 < 300);
    }
    sdl_audio_render(&tone, block, 4410, 0);
}

int qb_host_program(int argc, char **argv)
{
    SDL_Event quit;
    SD *key;
    unsigned turn;

    (void)argc, (void)argv;
    check_square_wave();
    qb_start();
    cn_write("HELLO FROM THE WORKER", 21);
    CHECK(frame_matches());

    gfx_screen(13);
    B_N1I2(10, 10);
    B_N2I2(300, 150);
    B_LINE(4, 0xFFFF, 0);
    CHECK(host_mode() == 0x13 && frame_matches());

    /* the comparison bites: a pixel changed behind the dirty flags is not on the screen */
    ((uint8_t *)host_pixels())[0] ^= 5;
    CHECK(!frame_matches_once());
    ((uint8_t *)host_pixels())[0] ^= 5;
    CHECK(frame_matches());

    /* the dirty-line protocol under load: the SDL thread presents while the program draws, and every write arrives */
    for (turn = 0; turn < 20000; turn++)
        gd_plot((turn * 37) % 320, (turn * 11) % 200, (turn * 7) & 255, 0);
    gd_palette_mix(5, 63, 0, 0);
    CHECK(frame_matches());

    /* SCREEN's pages: the host had no gd_set_pages, so a runtime that called it did not link. Drawing goes to the
       active page and only the visible one is on the screen. */
    gfx_screen(7);
    CHECK(gd_set_pages(1, 0) && !gd_set_pages(8, 0));
    gd_plot(3, 4, 9, 0);
    CHECK(host_pixels()[4 * 320 + 3] == 0 && frame_matches());
    CHECK(gd_set_pages(1, 1));
    CHECK(host_pixels()[4 * 320 + 3] == 9 && frame_matches());
    CHECK(gd_set_pages(0, 0) && host_pixels()[4 * 320 + 3] == 0);

    /* polling for a key or the clock asks for no frame: a loop that polled and drew in step with the presenter showed
       the same phase in every frame (NIBBLES' sparkles stood still on the Mac) */
    asked = sdl_sync_requests();
    for (turn = 0; turn < 200; turn++) {
        dev_key_ready();
        dev_clock();
        SDL_Delay(1);
    }
    CHECK(sdl_sync_requests() == asked);

    /* the fine clock was dev_clock() * 100: always a multiple of 100, so TIMER stepped by hundredths */
    {
        long fine, last = dev_clock_fine();
        int odd = 0;

        for (turn = 0; turn < 200; turn++) {
            fine = dev_clock_fine();
            odd |= fine % 100 != 0;
            CHECK(fine >= last || last - fine > 800000);
            last = fine;
            SDL_Delay(1);
        }
        CHECK(odd);
    }

    press(0, "a");
    press(SDLK_F1, 0);
    for (turn = 0; turn < 100 && !sdl_key_waiting(); turn++)
        sdl_sync_request();
    key = B_INKY();
    CHECK(key->len == 1 && key->ptr[0] == 'a');
    key = B_INKY();
    CHECK(key->len == 2 && key->ptr[0] == 0 && key->ptr[1] == 0x3B);

    dev_tone(440);
    for (turn = 0; turn < 100 && sdl_audio_current() != 440; turn++)
        SDL_Delay(10);
    CHECK(sdl_audio_current() == 440 || SDL_GetCurrentAudioDriver() == 0);
    dev_tone(0);

    puts(failures ? "failed" : "ok");
    fflush(stdout);
    SDL_zero(quit);
    quit.type = SDL_QUIT;
    SDL_PushEvent(&quit);
    dev_key_read(); /* the window is closing: the program ends here */
    puts("FAIL still running after the window closed");
    return 1;
}
