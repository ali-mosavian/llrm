/* sdl_fit: the largest 4:3 picture that fits the window, centred. */
#include <stdio.h>

#include "qbsdl.h"

static int failures;

static void check(int win_w, int win_h, int frame_w, int x, int y, int w, int h)
{
    SdlRect r = sdl_fit(win_w, win_h, frame_w);

    if (r.x != x || r.y != y || r.w != w || r.h != h) {
        printf("fit(%d,%d,%d) = %d,%d %dx%d, want %d,%d %dx%d\n", win_w, win_h, frame_w, r.x, r.y, r.w, r.h, x, y, w, h);
        failures++;
    }
}

int main(void)
{
    check(960, 720, 320, 0, 0, 960, 720);      /* exactly 3x */
    check(1000, 800, 320, 0, 25, 1000, 750);   /* width limits: fills it, bars above and below */
    check(1300, 720, 320, 170, 0, 960, 720);   /* height limits: bars left and right */
    check(959, 2000, 320, 0, 640, 959, 719);   /* not a whole scale: still fills the width */
    check(100, 100, 320, 0, 12, 100, 75);      /* smaller than 1x: shrinks */
    check(1280, 960, 640, 0, 0, 1280, 960);    /* a 640-wide mode is 640x480 at 4:3 */
    printf(failures ? "failed\n" : "ok\n");
    return failures != 0;
}
