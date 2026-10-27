/* The headless device platform through the device interface: text, keyboard, speaker, clock, tick, and the
   graphics screen in every mode. Exits 0, or 1 naming the check. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "dev.h"
#include "gfxdev.h"
#include "headless.h"
#include "video.h"

static int failures;
static int ticks;

void music_tick(void)
{
    ticks++;
}

#define CHECK(condition)                                                          \
    do {                                                                          \
        if (!(condition)) {                                                       \
            printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #condition);            \
            failures++;                                                           \
        }                                                                         \
    } while (0)

static void text_screen(void)
{
    char row[HOST_TEXT_COLUMNS + 1];
    unsigned x, set = 0, background = 0;

    headless_reset();
    CHECK(dev_text_size() == (25u << 8 | 80));
    dev_text_write(2, 3, "HELLO", 5, 0x1F);
    host_text_row(2, row);
    CHECK(memcmp(row + 3, "HELLO", 5) == 0 && row[2] == ' ' && row[8] == ' ');
    for (x = 0; x < 8; x++) {
        set += host_pixels()[(2 * 16 + 7) * host_width() + 3 * 8 + x] == 15;
        background += host_pixels()[(2 * 16 + 7) * host_width() + 3 * 8 + x] == 1;
    }
    CHECK(set > 0 && set + background == 8); /* white on blue: the 'H' bar row */
    dev_text_scroll(0, 24, 1, 0x07);
    host_text_row(1, row);
    CHECK(memcmp(row + 3, "HELLO", 5) == 0);
    host_text_row(24, row);
    CHECK(row[0] == ' ');
    dev_text_move(5, 7);
    CHECK(dev_text_cursor() == (5u << 8 | 7));
    dev_text_scroll(0, 24, 0, 0x07);
    host_text_row(1, row);
    CHECK(row[3] == ' '); /* no lines clears */
}

static void keyboard(void)
{
    headless_reset();
    CHECK(!dev_key_ready());
    CHECK(dev_key_read() == 26); /* nothing will type one */
    headless_type("ab");
    headless_type_extended(0x3B);
    CHECK(dev_key_ready() && dev_key_peek_scan() == -1);
    CHECK(dev_key_read() == 'a' && dev_key_read() == 'b');
    CHECK(dev_key_peek_scan() == 0x3B);
    dev_key_discard();
    CHECK(!dev_key_ready() && dev_key_peek_scan() == -1);
    headless_type_extended(0x48);
    CHECK(dev_key_read() == 0 && dev_key_read() == 0x48);
    headless_set_end_of_input(1);
    CHECK(dev_key_ready() && dev_key_read() == 26 && dev_key_read() == 26);
}

static void speaker_and_clock(void)
{
    unsigned year, month, day, hour, minute, second;

    headless_reset();
    dev_tone(440);
    headless_clock_advance(25);
    dev_tone(0);
    CHECK(headless_tone_count() == 2 && headless_tone(0).hertz == 440 && headless_tone(1).hertz == 0);
    CHECK(headless_tone(1).clock - headless_tone(0).clock == 25);

    headless_clock_set(100, 1);
    CHECK(dev_clock() == 100 && dev_clock() == 101); /* a wait loop ends */
    CHECK(dev_time_set(23, 59, 58) == 0);
    headless_clock_set(8639900, 0);
    dev_time(&hour, &minute, &second);
    CHECK(hour == 23 && minute == 59 && second == 59);
    CHECK(dev_time_set(24, 0, 0) != 0 && dev_date_set(2001, 13, 1) != 0);
    CHECK(dev_date_set(1979, 12, 31) != 0); /* before DOS's epoch */
    CHECK(dev_date_set(2024, 2, 29) == 0);
    dev_date(&year, &month, &day);
    CHECK(year == 2024 && month == 2 && day == 29);

    headless_reset();
    dev_tick_start();
    headless_run(100);
    CHECK(ticks == 18); /* 18.2 a second */
    dev_tick_hold();
    headless_run(100); /* interrupts off */
    CHECK(ticks == 18);
    dev_tick_release();
    dev_tick_stop();
    headless_run(100);
    CHECK(ticks == 18);
    CHECK(dev_stdout_is_screen() && !dev_printer_ready());
    headless_set_printer_ready(1);
    CHECK(dev_printer_ready());
}

/* A picture of known pixels in the box, with the array the adapter would give GET. */
static unsigned expect_color(unsigned x, unsigned y, unsigned mask)
{
    return (x * 7 + y * 3 + (x >> 2) * 5) & mask;
}

static void graphics_mode(unsigned bios, unsigned width, unsigned height, unsigned colors)
{
    unsigned mask = colors - 1, x, y;
    GdFill fill;
    uint8_t array[1024 * 16], again[1024 * 16];

    headless_reset();
    CHECK(gd_set_mode(bios) == 1);
    CHECK(host_width() == width && host_height() == height && host_mode() == bios);
    for (y = 0; y < 40; y++)
        for (x = 0; x < 48; x++)
            gd_plot(x + 5, y + 2, expect_color(x, y, mask), 0);
    for (y = 0; y < 40; y++)
        for (x = 0; x < 48; x++)
            if (gd_read(x + 5, y + 2) != expect_color(x, y, mask)) {
                printf("FAIL mode %02X pixel %u,%u\n", bios, x, y);
                failures++;
                return;
            }
    CHECK(gd_read(0, 0) == 0);

    /* a box, an operation, a line */
    gd_fill_select(&fill, colors - 1, 0);
    fill.box(&fill, 100, 50, 20, 10);
    CHECK(gd_read(100, 50) == mask && gd_read(119, 59) == mask && gd_read(120, 50) == 0 && gd_read(100, 60) == 0);
    gd_fill_select(&fill, 1, 3);
    fill.box(&fill, 100, 50, 20, 10);
    CHECK(gd_read(110, 55) == (mask ^ 1));
    gd_fill_select(&fill, colors - 1, 0);
    fill.line(&fill, 10, 100, 30, 0, 1);
    CHECK(gd_read(10, 100) == mask && gd_read(40, 100) == mask && gd_read(41, 100) == 0);
    fill.line(&fill, 10, 110, 0, 20, 1);
    CHECK(gd_read(10, 110) == mask && gd_read(10, 130) == mask);
    fill.line(&fill, 50, 150, 20, 20, -1);
    CHECK(gd_read(50, 150) == mask && gd_read(70, 130) == mask);
    fill.style = 0xF0F0;
    fill.line(&fill, 0, 160, 15, 0, 1);
    CHECK(gd_read(0, 160) == mask && gd_read(3, 160) == mask && gd_read(4, 160) == 0 && gd_read(8, 160) == mask);

    /* GET then PUT back: the same pixels; PUT with xor twice: the original */
    {
        unsigned bits = bios == 4 ? 2 : bios == 6 ? 1 : bios == 0x13 ? 8 : 1;
        unsigned planes = (bios == 4 || bios == 6 || bios == 0x13) ? 1 : 4;
        unsigned bytes = (37 * bits + 7) / 8;

        memset(array, 0xAA, sizeof array);
        gd_get(8, 3, 37, 11, array);
        CHECK(array[0] != 0xAA);
        gd_put(60, 200 - 30, 37, 11, array, 0, 0);
        for (y = 0; y < 11; y++)
            for (x = 0; x < 37; x++)
                if (gd_read(60 + x, 170 + y) != gd_read(8 + x, 3 + y)) {
                    printf("FAIL mode %02X put %u,%u\n", bios, x, y);
                    failures++;
                    return;
                }
        gd_get(60, 170, 37, 11, again);
        CHECK(memcmp(array, again, (size_t)planes * bytes * 11) == 0);
        gd_put(60, 170, 37, 11, array, 3, 0);
        CHECK(gd_read(60, 170) == 0 && gd_read(70, 175) == 0);
        gd_put(60, 170, 37, 11, array, 0, 1);
        CHECK(gd_read(60, 170) == (gd_read(8, 3) ^ mask));
    }

    /* searches */
    gd_fill_select(&fill, 1, 0);
    fill.box(&fill, 200, 20, 10, 1);
    CHECK(gd_search(150, 250, 20, 1, 1, 1) == 200);
    CHECK(gd_search(250, 150, 20, 1, 1, 1) == 209);
    CHECK(gd_search(200, 250, 20, 1, 1, 0) == 210);
    CHECK(gd_search(60, 100, 20, 1, 1, 1) == -1);
    {
        int left, right;

        gd_plot(100, 180, 1, 0);
        gd_plot(300, 180, 1, 0);
        CHECK(gd_run(150, 250, 180, 1, 7, 310, &left, &right) == 1 && left == 101 && right == 299);
    }

    /* rows move */
    gd_move_rows(0, 2, 5);
    CHECK(gd_read(5, 0) == expect_color(0, 0, mask) && gd_read(30, 4) == expect_color(25, 4, mask));
}

static void palette(void)
{
    uint8_t rgb[256][3];

    headless_reset();
    host_colors(rgb);
    CHECK(rgb[0][0] == 0 && rgb[7][0] == 0xAA && rgb[15][0] == 0xFF && rgb[6][0] == 0xAA && rgb[6][1] == 0x55 && rgb[8][0] == 0x55);
    gd_set_mode(0x13);
    host_colors(rgb);
    CHECK(rgb[1][2] == 0xAA && rgb[1][0] == 0 && rgb[4][0] == 0xAA);
    gd_palette_mix(200, 63, 0, 21);
    host_colors(rgb);
    CHECK(rgb[200][0] == 255 && rgb[200][1] == 0 && rgb[200][2] == 85);
    gd_set_mode(0x10);
    gd_palette(3, 0x3F);
    host_colors(rgb);
    CHECK(rgb[3][0] == 255 && rgb[3][1] == 255 && rgb[3][2] == 255);
    gd_set_mode(4);
    gd_cga_color(1, 0);
    host_colors(rgb);
    CHECK(rgb[0][2] == 0xAA && rgb[1][1] == 0xAA && rgb[1][0] == 0 && rgb[2][0] == 0xAA);
}

static void dump(const char *path)
{
    FILE *file;
    char header[32] = {0};
    unsigned width, height;
    long size;

    headless_reset();
    gd_set_mode(0x13);
    gd_plot(1, 0, 4, 0);
    CHECK(host_dump_ppm(path) == 0);
    file = fopen(path, "rb");
    CHECK(file != NULL);
    if (!file)
        return;
    CHECK(fscanf(file, "%2s %u %u", header, &width, &height) == 3 && strcmp(header, "P6") == 0 && width == 320 && height == 200);
    fseek(file, 0, SEEK_END);
    size = ftell(file);
    CHECK(size == (long)(strlen("P6\n320 200\n255\n") + 320 * 200 * 3));
    fseek(file, (long)strlen("P6\n320 200\n255\n") + 3, SEEK_SET);
    CHECK(fgetc(file) == 0xAA && fgetc(file) == 0 && fgetc(file) == 0); /* pixel 1,0 is red */
    fclose(file);
    remove(path);
}

int main(int argc, char **argv)
{
    static const struct {
        unsigned bios, width, height, colors;
    } modes[] = {{4, 320, 200, 4}, {6, 640, 200, 2}, {0x0D, 320, 200, 16}, {0x0E, 640, 200, 16},
                 {0x10, 640, 350, 16}, {0x12, 640, 480, 16}, {0x13, 320, 200, 256}};
    unsigned at;

    text_screen();
    keyboard();
    speaker_and_clock();
    for (at = 0; at < sizeof modes / sizeof modes[0]; at++)
        graphics_mode(modes[at].bios, modes[at].width, modes[at].height, modes[at].colors);
    palette();
    dump(argc > 1 ? argv[1] : "headless_dump.ppm");
    puts(failures ? "failed" : "ok");
    return failures != 0;
}
