/* The device interface's keyboard, speaker and clock over the sdl modules; the screen is host/video.c, the tick
   host/tick.c. Every call that can wait looks for the window's closing, and the ones a program polls in a loop (keys,
   the clock) let a frame reach the screen. */
#define _POSIX_C_SOURCE 200809L
#include <time.h>

#include "dev.h"
#include "llrm_os.h"
#include "qbsdl.h"

enum { TIME_WORDS = 7, DAY = 8640000 };

/* The window was closed: the program ends as END does. */
static void leave_if_closed(void)
{
    if (sdl_interrupted())
        llrm_os_exit(0);
}

/* The first frame after a mode change is shown before the program draws on it. */
void host_video_mode_changed(void)
{
    sdl_sync_request();
}

int dev_key_ready(void)
{
    leave_if_closed();
    return sdl_key_waiting();
}

unsigned char dev_key_read(void)
{
    int key;

    for (;;) {
        leave_if_closed();
        key = sdl_key_take();
        if (key >= 0)
            return (unsigned char)key;
        sdl_sync_request();
    }
}

int dev_key_peek_scan(void)
{
    return sdl_key_peek_scan();
}

void dev_key_discard(void)
{
    if (sdl_key_take() == 0)
        sdl_key_take();
}

void dev_tone(unsigned hertz)
{
    sdl_audio_tone(hertz);
}

static void now(unsigned words[TIME_WORDS])
{
    unsigned char data[2 * TIME_WORDS];
    unsigned at;

    llrm_os_local_time(data);
    for (at = 0; at < TIME_WORDS; at++)
        words[at] = (unsigned)data[2 * at] | (unsigned)data[2 * at + 1] << 8;
}

long dev_clock(void)
{
    unsigned t[TIME_WORDS];

    leave_if_closed();
    now(t);
    return (((long)t[3] * 60 + t[4]) * 60 + t[5]) * 100 + t[6];
}

long dev_clock_fine(void)
{
    struct timespec at;
    struct tm local;

    leave_if_closed();
    clock_gettime(CLOCK_REALTIME, &at);
    localtime_r(&at.tv_sec, &local);
    return ((long)(local.tm_hour * 60 + local.tm_min) * 60 + local.tm_sec) * 10000 +
           at.tv_nsec / 100000;
}

void dev_date(unsigned *year, unsigned *month, unsigned *day)
{
    unsigned t[TIME_WORDS];

    now(t);
    *year = t[0];
    *month = t[1];
    *day = t[2];
}

void dev_time(unsigned *hour, unsigned *minute, unsigned *second)
{
    unsigned t[TIME_WORDS];

    now(t);
    *hour = t[3];
    *minute = t[4];
    *second = t[5];
}

/* A program does not set the host's clock. */
int dev_date_set(unsigned year, unsigned month, unsigned day)
{
    (void)year, (void)month, (void)day;
    return 1;
}

int dev_time_set(unsigned hour, unsigned minute, unsigned second)
{
    (void)hour, (void)minute, (void)second;
    return 1;
}

int dev_stdout_is_screen(void)
{
    return 1;
}

int dev_printer_ready(void)
{
    return 0;
}
