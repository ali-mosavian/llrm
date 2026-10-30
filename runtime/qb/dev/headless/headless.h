/* The headless device platform: the device interface over the host framebuffer (host/video.h) with a keyboard
   that is a script, a speaker that is a log, and a clock that is a number. Tests drive a program with it and
   read the screen. Single-threaded: the program's calls and the test's are one thread. */
#ifndef QB_HEADLESS_H
#define QB_HEADLESS_H

#include <stddef.h>

/* Back to the start of a program: text mode, no keys, no tones, the clock at 12:00:00.00 on 2000-01-01. */
void headless_reset(void);

/* Keys: text typed as bytes, and extended keys by scan code. */
void headless_type(const char *text);
void headless_type_extended(unsigned scan);

/* With `yes` the keyboard is always ready (at the end of redirected input: Ctrl-Z); by default it is ready when a key
   is waiting, and a read that would wait for one returns Ctrl-Z, since nothing will type it. */
void headless_set_end_of_input(int yes);

/* The speaker: each change of tone, in the order they came, with the clock when it came. */
typedef struct HeadlessTone {
    unsigned hertz;
    long clock;
} HeadlessTone;
size_t headless_tone_count(void);
HeadlessTone headless_tone(size_t index);

/* The clock, in hundredths of a second since midnight. `step` is added to it by each dev_clock call, so a wait
   loop ends; 0 stops it. */
void headless_clock_set(long hundredths, long step);
/* With `yes` the clock runs by itself in real time from where it is (a program that times itself ends); by default it is
   only what a test makes of it. */
void headless_set_real_time(int yes);
void headless_clock_advance(long hundredths);
/* Calls music_tick once for each tick of 1/18.2 second that `hundredths` make, while the tick is on and not held. */
void headless_run(long hundredths);

/* With `yes` the tick runs by itself in real time (18.2 a second) once a program starts it, as the timer interrupt does;
   by default it runs only in headless_run, so a test sees the same notes every time. */
void headless_set_tick_thread(int yes);

/* Whether standard output is the screen (default), and whether the printer is ready (default not). */
void headless_set_stdout_is_screen(int yes);
void headless_set_printer_ready(int yes);

#endif
