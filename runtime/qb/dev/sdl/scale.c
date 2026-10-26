/* Where the picture goes in a window of any size. */
#include "qbsdl.h"

/* The picture is shown at 4:3 as the adapter does, `frame_width` by three quarters of it, scaled to the largest size that
   fits the window and centred. */
SdlRect sdl_fit(int window_width, int window_height, int frame_width)
{
    int width = frame_width, height = frame_width * 3 / 4;
    SdlRect r;

    if ((long)window_width * height <= (long)window_height * width) {
        r.w = window_width;
        r.h = (int)((long)window_width * height / width);
    } else {
        r.h = window_height;
        r.w = (int)((long)window_height * width / height);
    }
    r.x = (window_width - r.w) / 2;
    r.y = (window_height - r.h) / 2;
    return r;
}
