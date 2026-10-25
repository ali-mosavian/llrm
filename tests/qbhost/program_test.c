/* QB statements on the headless device: the runtime's own entry points (PRINT's console, INKEY$, BEEP, SOUND, PLAY,
   TIMER, TIME$, DATE$, SCREEN, LINE, CIRCLE, PAINT, GET and PUT), called as a compiled program calls them, with the
   screen, the keys, the speaker and the clock read through headless.h. Exits 0, or 1 naming the check. */
#include <stdio.h>
#include <string.h>

#include "ad.h"
#include "console.h"
#include "gfx.h"
#include "gfxdev.h"
#include "headless.h"
#include "llrm_os.h"
#include "nhstutil.h"
#include "rtinit.h"
#include "video.h"

extern void qb_start(void);
extern SD *B_INKY(void);
extern float *B_TIMR(void);
extern SD *B_FTIM(void);
extern SD *B_FDAT(void);
extern void B_SDAT(SD *text);
extern void B_BEEP(void);
extern void B_SPLY(SD *commands);
extern void B_SOND(short hertz, float duration);
extern void B_N1I2(short x, short y);
extern void B_N2I2(short x, short y);
extern void B_PSTC(short color);
extern void B_PSET(void);
extern void B_LINE(short color, short style, short how);
extern void B_CIRC(float radius, short color);
extern void B_PAIN(short fill, short border);
extern void B_GGET(qb_data_ptr data, const AD *ad);
extern void B_GPUT(qb_data_ptr data, const AD *ad, short how);

static int failures;

#define CHECK(condition)                                                \
    do {                                                                \
        if (!(condition)) {                                             \
            printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #condition);  \
            failures++;                                                 \
        }                                                               \
    } while (0)

static SD *string_of(const char *text)
{
    char *data;
    SD *sd = str_tmp((uword)strlen(text), &data);

    memcpy(data, text, strlen(text));
    return sd;
}

static void say(const char *text)
{
    cn_write(text, (unsigned)strlen(text));
}

static int row_is(unsigned row, const char *text)
{
    char got[HOST_TEXT_COLUMNS + 1];

    host_text_row(row, got);
    return strncmp(got, text, strlen(text)) == 0;
}

static unsigned cell_attribute(unsigned row, unsigned column)
{
    /* the foreground of a glyph's lit pixel and the background beside it say the attribute */
    const uint8_t *pixels = host_pixels();
    unsigned lit = 0, dark = 0, x, y;

    for (y = 0; y < HOST_CELL_HEIGHT; y++)
        for (x = 0; x < HOST_CELL_WIDTH; x++) {
            unsigned value = pixels[(row * HOST_CELL_HEIGHT + y) * host_width() + column * HOST_CELL_WIDTH + x];

            if (value > dark)
                lit = value;
            if (x == 0 && y == 0)
                dark = value;
        }
    return lit << 4 | dark;
}

static void printing(void)
{
    char numbered[16];
    int line;

    headless_reset();
    say("HELLO");
    cn_crlf();
    say("WORLD");
    CHECK(row_is(0, "HELLO") && row_is(1, "WORLD") && cn_pos() == 5 && cn_line() == 1);
    cn_locate(5, 10, -1);
    say("X");
    CHECK(row_is(4, "         X"));
    cn_color(14, 1);
    say("Y");
    CHECK(cell_attribute(4, 10) == (1u << 4 | 14) || cell_attribute(4, 10) == (14u << 4 | 1));
    cn_cls();
    CHECK(row_is(0, "     ") && row_is(1, "     ") && cn_line() == 0);
    cn_color(7, 0);
    for (line = 0; line < 30; line++) {
        sprintf(numbered, "L%02d", line);
        say(numbered);
        cn_crlf();
    }
    CHECK(row_is(0, "L07") && row_is(22, "L29") && row_is(23, "   "));
}

static void keys_and_sound(void)
{
    SD *key;

    headless_reset();
    key = B_INKY();
    CHECK(key->len == 0);
    headless_type("a");
    key = B_INKY();
    CHECK(key->len == 1 && key->ptr[0] == 'a');
    headless_type_extended(0x3B);
    key = B_INKY();
    CHECK(key->len == 2 && key->ptr[0] == 0 && key->ptr[1] == 0x3B);
    CHECK(B_INKY()->len == 0);

    headless_reset();
    headless_clock_set(0, 1);
    B_BEEP();
    CHECK(headless_tone_count() == 2 && headless_tone(0).hertz == 800 && headless_tone(1).hertz == 0);
    CHECK(headless_tone(1).clock - headless_tone(0).clock >= 25);

    /* in a graphics mode BEEP was silent (the graphics console passed the bell over) */
    gfx_screen(13);
    headless_reset();
    headless_clock_set(0, 1);
    B_BEEP();
    CHECK(headless_tone_count() == 2 && headless_tone(0).hertz == 800);
}

static void music(void)
{
    size_t at;

    headless_reset();
    B_SPLY(string_of("MB T120 O4 C4 D4"));
    CHECK(headless_tone_count() == 0); /* the first note starts at the next tick */
    headless_run(10);
    CHECK(headless_tone_count() == 1 && headless_tone(0).hertz == 524);
    headless_run(100);
    CHECK(headless_tone_count() >= 3 && headless_tone(2).hertz == 588);
    headless_run(200);
    CHECK(headless_tone(headless_tone_count() - 1).hertz == 0);

    /* foreground SOUND waits for the note before it, so the tick must run by itself */
    headless_reset();
    headless_set_tick_thread(1);
    B_SOND(440, 1);
    B_SOND(880, 1);
    B_SOND(660, 1);
    llrm_os_sleep(500);
    headless_set_tick_thread(0);
    CHECK(headless_tone_count() >= 4 && headless_tone(0).hertz == 440 && headless_tone(1).hertz == 0 &&
          headless_tone(2).hertz == 880);
}

static void clock(void)
{
    float *seconds;
    SD *text;

    headless_reset();
    headless_clock_set(4321, 0);
    seconds = B_TIMR();
    CHECK(*seconds > 43.20f && *seconds < 43.22f);
    headless_clock_set((12L * 3600 + 34 * 60 + 56) * 100, 0);
    text = B_FTIM();
    CHECK(text->len == 8 && memcmp(text->ptr, "12:34:56", 8) == 0);
    B_SDAT(string_of("02-29-2024"));
    text = B_FDAT();
    CHECK(text->len == 10 && memcmp(text->ptr, "02-29-2024", 10) == 0);
}

static void drawing(void)
{
    static struct {
        AD ad;
        DM more[0];
    } box;
    static short storage[256];
    unsigned x, y;

    headless_reset();
    gfx_screen(13);
    CHECK(host_mode() == 0x13 && host_width() == 320);
    B_N1I2(10, 10);
    B_N2I2(100, 10);
    B_LINE(4, 0xFFFF, 0);
    CHECK(gd_read(10, 10) == 4 && gd_read(55, 10) == 4 && gd_read(100, 10) == 4 && gd_read(101, 10) == 0);

    B_N1I2(160, 100);
    B_CIRC(30.0f, 4);
    CHECK(gd_read(190, 100) == 4 && gd_read(130, 100) == 4 && gd_read(160, 75) == 4 && gd_read(160, 125) == 4);
    CHECK(gd_read(160, 100) == 0);
    B_N1I2(160, 100);
    B_PAIN(2, 4);
    CHECK(gd_read(160, 100) == 2 && gd_read(175, 110) == 2 && gd_read(190, 100) == 4 && gd_read(200, 100) == 0);

    B_N1I2(150, 90);
    B_PSTC(9);
    CHECK(gd_read(150, 90) == 9);
    B_PSET();
    CHECK(gd_read(150, 90) == 15);

    /* GET the circle's box, PUT it elsewhere */
    box.ad.dims = 1;
    box.ad.elem = 2;
    box.ad.data = storage;
    box.ad.dm[0].count = 256;
    B_N1I2(150, 90);
    B_N2I2(169, 99);
    B_GGET(storage, &box.ad);
    CHECK(storage[0] == 20 * 8 && storage[1] == 10);
    B_N1I2(20, 150);
    B_GPUT(storage, &box.ad, 3);
    for (y = 0; y < 10; y++)
        for (x = 0; x < 20; x++)
            if (gd_read(20 + x, 150 + y) != gd_read(150 + x, 90 + y)) {
                printf("FAIL put %u,%u\n", x, y);
                failures++;
                return;
            }
}

int main(void)
{
    qb_start();
    printing();
    keys_and_sound();
    music();
    clock();
    drawing();
    puts(failures ? "failed" : "ok");
    return failures != 0;
}
