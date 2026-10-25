/* The screen of a host: one framebuffer of pixel values (a byte each) for the text screen and every
   graphics mode, the palette that turns a value into a colour, and a flag for each line a write has
   touched. The graphics (gd_*) and text (dev_text_*) operations of the device interface are in video.c;
   headless and sdl add the keyboard, the speaker and the clock, and sdl a window that reads this.

   Threads: the program writes pixels, then sets the line's flag (release); a presenter exchanges the flag
   (acquire), then copies the line. A write that lands during the copy sets the flag again, so no update is
   lost; a line may tear. */
#ifndef QB_HOST_VIDEO_H
#define QB_HOST_VIDEO_H

#include <stdatomic.h>
#include <stdint.h>

enum {
    HOST_MAX_WIDTH = 640,
    HOST_MAX_HEIGHT = 480,
    HOST_TEXT_COLUMNS = 80,
    HOST_TEXT_ROWS = 25,
    HOST_CELL_WIDTH = 8,
    HOST_CELL_HEIGHT = 16
};

/* The BIOS modes the screen has. */
enum { HOST_MODE_TEXT = 3, HOST_MODE_CGA4 = 4, HOST_MODE_CGA2 = 6, HOST_MODE_EGA320 = 0x0D };

/* The flag of each line of the frame; see above. */
/* The SDL thread holds this while it reads the mode, palette, cursor and pixels (host_colors, host_cursor and the
   conversion of dirty lines); the program holds it while it changes them. */
void host_state_lock(void);
void host_state_unlock(void);

extern _Atomic uint8_t host_dirty[HOST_MAX_HEIGHT];

/* Text mode, cleared, as at the start of a program. */
void host_video_reset(void);

/* The frame: its size, the BIOS mode, and the pixel values, `width` to a row. */
unsigned host_width(void);
unsigned host_height(void);
unsigned host_mode(void);
const uint8_t *host_pixels(void);
/* Counts the mode changes: a presenter that sees it move starts a frame of the new size. */
unsigned host_mode_serial(void);

/* The colour of each pixel value, 8 bits a channel. */
void host_colors(uint8_t rgb[256][3]);

/* The text cursor: row, column, and whether it shows. */
void host_cursor(unsigned *row, unsigned *column, int *visible);

/* The text screen's characters, one row a line, as a test reads them. */
void host_text_row(unsigned row, char out[HOST_TEXT_COLUMNS + 1]);

/* The frame as a binary PPM; 0, or -1 if the file would not write. */
int host_dump_ppm(const char *path);

#endif
