#define _POSIX_C_SOURCE 200809L
#include <string.h>
#include <time.h>

#include "dev.h"
#include "headless.h"
#include "tick.h"
#include "video.h"

enum {
    KEY_BYTES = 256,
    TONE_LOG = 256,
    CTRL_Z = 26,
    DAY = 8640000,
    TICK_UNITS = 1000, /* a tick is 1000 units; a hundredth is 182 of them (18.2 ticks a second) */
    UNITS_PER_HUNDREDTH = 182,
    HUNDREDTHS_PER_HOUR = 360000,
    HUNDREDTHS_PER_MINUTE = 6000
};

/* The keys are pairs: the byte, and for an extended key (a byte of 0) its scan code. */
static unsigned char keys[KEY_BYTES];
static size_t key_head, key_tail;
static int scan_pending, end_of_input;

static HeadlessTone tones[TONE_LOG];
static size_t tone_count;

static long clock_now, clock_step;
/* In real time the clock is what it was set to plus the time since: a program that times itself (NIBBLES' speed loop)
   ends. */
static int real_time;
static long real_epoch;

static long monotonic_hundredths(void)
{
    struct timespec now;

    clock_gettime(CLOCK_MONOTONIC, &now);
    return (long)now.tv_sec * 100 + now.tv_nsec / 10000000;
}

/* The clock is set to `hundredths` now. */
static void rebase(long hundredths)
{
    clock_now = hundredths % DAY;
    real_epoch = monotonic_hundredths();
}

static void refresh(void)
{
    long now = monotonic_hundredths();

    if (real_time) {
        clock_now = (clock_now + now - real_epoch) % DAY;
        real_epoch = now;
    }
}

void headless_set_real_time(int yes)
{
    real_time = yes;
    real_epoch = monotonic_hundredths();
}
static unsigned date_year, date_month, date_day;
static long tick_remainder;
static int on_screen = 1, printer = 0;

void headless_reset(void)
{
    host_video_reset();
    key_head = key_tail = 0;
    scan_pending = 0;
    end_of_input = 0;
    tone_count = 0;
    clock_now = 12L * HUNDREDTHS_PER_HOUR;
    clock_step = 0;
    date_year = 2000;
    date_month = 1;
    date_day = 1;
    tick_remainder = 0;
    on_screen = 1;
    printer = 0;
}

static void push(unsigned char byte)
{
    keys[key_tail++ % KEY_BYTES] = byte;
}

void headless_type(const char *text)
{
    while (*text)
        push((unsigned char)*text++);
}

void headless_set_end_of_input(int yes)
{
    end_of_input = yes;
}

void headless_type_extended(unsigned scan)
{
    push(0);
    push((unsigned char)scan);
}

static size_t waiting(void)
{
    return key_tail - key_head;
}

int dev_key_ready(void)
{
    return waiting() != 0 || end_of_input;
}

unsigned char dev_key_read(void)
{
    if (!waiting())
        return CTRL_Z;
    return keys[key_head++ % KEY_BYTES];
}

int dev_key_peek_scan(void)
{
    if (waiting() < 2 || keys[key_head % KEY_BYTES] != 0)
        return -1;
    return keys[(key_head + 1) % KEY_BYTES];
}

void dev_key_discard(void)
{
    if (!waiting())
        return;
    if (keys[key_head % KEY_BYTES] == 0 && waiting() >= 2)
        key_head++;
    key_head++;
}

void dev_tone(unsigned hertz)
{
    if (tone_count < TONE_LOG) {
        tones[tone_count].hertz = hertz;
        tones[tone_count].clock = clock_now;
        tone_count++;
    }
}

size_t headless_tone_count(void)
{
    return tone_count;
}

HeadlessTone headless_tone(size_t index)
{
    return tones[index];
}

void headless_clock_set(long hundredths, long step)
{
    clock_now = hundredths % DAY;
    clock_step = step;
}

void headless_clock_advance(long hundredths)
{
    clock_now = (clock_now + hundredths) % DAY;
}

long dev_clock(void)
{
    long now;

    refresh();
    now = clock_now;

    clock_now = (clock_now + clock_step) % DAY;
    return now;
}

/* No clock finer than hundredths here. */
long dev_clock_fine(void)
{
    return dev_clock() * 100;
}

void dev_date(unsigned *year, unsigned *month, unsigned *day)
{
    *year = date_year;
    *month = date_month;
    *day = date_day;
}

void dev_time(unsigned *hour, unsigned *minute, unsigned *second)
{
    refresh();
    *hour = (unsigned)(clock_now / HUNDREDTHS_PER_HOUR);
    *minute = (unsigned)(clock_now / HUNDREDTHS_PER_MINUTE % 60);
    *second = (unsigned)(clock_now / 100 % 60);
}

int dev_date_set(unsigned year, unsigned month, unsigned day)
{
    if (year < 1980 || year > 2099 || month < 1 || month > 12 || day < 1 || day > 31)
        return 1;
    date_year = year;
    date_month = month;
    date_day = day;
    return 0;
}

int dev_time_set(unsigned hour, unsigned minute, unsigned second)
{
    if (hour > 23 || minute > 59 || second > 59)
        return 1;
    rebase((long)hour * HUNDREDTHS_PER_HOUR + (long)minute * HUNDREDTHS_PER_MINUTE + (long)second * 100);
    return 0;
}

void headless_run(long hundredths)
{
    clock_now = (clock_now + hundredths) % DAY;
    tick_remainder += hundredths * UNITS_PER_HUNDREDTH;
    while (tick_remainder >= TICK_UNITS) {
        tick_remainder -= TICK_UNITS;
        host_tick_fire();
    }
}

int dev_stdout_is_screen(void)
{
    return on_screen;
}

int dev_printer_ready(void)
{
    return printer;
}

void headless_set_tick_thread(int yes)
{
    host_tick_set_thread(yes);
}

void headless_set_stdout_is_screen(int yes)
{
    on_screen = yes;
}

void headless_set_printer_ready(int yes)
{
    printer = yes;
}
