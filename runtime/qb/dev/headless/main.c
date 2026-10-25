/* A compiled QB program on the headless device: main runs it, prints the text screen it left (the rows that are not
   blank) and leaves with its exit code. */
#include <stdio.h>
#include <string.h>

#include "llrm_os.h"
#include "video.h"

extern int qb_host_program(int argc, char **argv);

static void screen_dump(int code)
{
    unsigned row;
    char text[HOST_TEXT_COLUMNS + 1];

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

int main(int argc, char **argv)
{
    llrm_os_set_arguments(argc, argv);
    llrm_os_set_exit_hook(screen_dump);
    return qb_host_program(argc, argv);
}
