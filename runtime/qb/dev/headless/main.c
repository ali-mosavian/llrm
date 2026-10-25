/* A compiled QB program on the headless device: main runs it, prints the text screen it left (the rows that are not
   blank) and leaves with its exit code. The environment drives it: QB_HEADLESS_KEYS is typed, a key every
   QB_HEADLESS_GAP milliseconds (default 700) from the program's start (\r, \n, \t and \\ are escapes), QB_HEADLESS_SECONDS ends a program that has not finished by then, with the screen it
   has, and QB_HEADLESS_PPM names a file the final screen is written to as a PPM image. */
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "headless.h"
#include "llrm_os.h"
#include "video.h"

extern int qb_host_program(int argc, char **argv);

static void screen_dump(int code)
{
    unsigned row;
    char text[HOST_TEXT_COLUMNS + 1];
    const char *image = getenv("QB_HEADLESS_PPM");

    if (image)
        host_dump_ppm(image);
    for (row = 0; row < HOST_TEXT_ROWS; row++) {
        size_t length;

        host_text_row(row, text);
        for (length = strlen(text); length && text[length - 1] == ' '; length--)
            ;
        if (length)
            printf("%.*s\n", (int)length, text);
    }
    fflush(stdout);
    (void)code;
}

static void out_of_time(int signal_number)
{
    (void)signal_number;
    screen_dump(0);
    _exit(124);
}

static const char *script;

/* A key at a time, from a thread: a program that clears the keyboard when it starts would lose them all at once. */
static void *typist(void *unused)
{
    const char *keys = script;
    const char *gap = getenv("QB_HEADLESS_GAP");
    char one[2] = {0, 0};

    (void)unused;
    for (; *keys; keys++) {
        char c = *keys;

        if (c == '\\' && keys[1]) {
            keys++;
            c = *keys == 'r' || *keys == 'n' ? '\r' : *keys == 't' ? '\t' : *keys;
        }
        usleep((useconds_t)(gap ? atoi(gap) : 700) * 1000);
        one[0] = c;
        headless_type(one);
    }
    return 0;
}

int main(int argc, char **argv)
{
    const char *keys = getenv("QB_HEADLESS_KEYS"), *seconds = getenv("QB_HEADLESS_SECONDS");

    llrm_os_set_arguments(argc, argv);
    headless_reset();
    llrm_os_set_exit_hook(screen_dump);
    headless_set_tick_thread(1);
    headless_set_real_time(1);
    if (keys) {
        pthread_t thread;

        script = keys;
        pthread_create(&thread, 0, typist, 0);
    }
    if (seconds) {
        signal(SIGALRM, out_of_time);
        alarm((unsigned)atoi(seconds));
    }
    return qb_host_program(argc, argv);
}
