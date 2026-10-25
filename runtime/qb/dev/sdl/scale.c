/* Where the picture goes in a window of any size. */
#include "qbsdl.h"

/* The picture is shown at 4:3 as the adapter does, `frame_width` by three quarters of it, times the largest whole number
   that fits; a window smaller than one times shows the corner. */
SdlRect sdl_fit(int window_width, int window_height, int frame_width)
{
    int width = frame_width, height = frame_width * 3 / 4;
    int by_width = window_width / width, by_height = window_height / height;
    int scale = by_width < by_height ? by_width : by_height;
    SdlRect r;

    if (scale < 1)
        scale = 1;
    r.w = width * scale;
    r.h = height * scale;
    r.x = (window_width - r.w) / 2;
    r.y = (window_height - r.h) / 2;
    if (r.x < 0)
        r.x = 0;
    if (r.y < 0)
        r.y = 0;
    return r;
}
