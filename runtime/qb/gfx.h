/* The graphics screen (QB rt/gr*.asm): the mode in use, the colours, the pixels
   drawn through the OS layer, and the points a statement is given. */
#ifndef QB_GFX_H
#define QB_GFX_H

#include "qb.h"

/* A screen mode of 16 colours in planes. */
typedef struct GfxMode {
    byte number;           /* of SCREEN */
    byte bios;             /* the BIOS's own */
    unsigned width, height;
    byte cell_height;      /* of a text character */
    double aspect;         /* height of a pixel over its width, times 1 */
} GfxMode;

extern const GfxMode *gfx_current;
extern byte gfx_foreground, gfx_background;

/* The two points of the statement being run, as B$N1I2 and B$N2I2 gave them. */
extern int gfx_x1, gfx_y1, gfx_x2, gfx_y2;

/* SCREEN: sets the mode, or returns to text with 0.  Not a mode this has is an
   Illegal function call. */
void gfx_screen(int mode);

/* The background is colour 0 made to look like another: the palette's first
   entry is given the colour of that attribute (the adapter's default one). */
void gfx_set_background(byte color);

/* A colour argument: -1 is the foreground; out of range is an error. */
byte gfx_color(int color);

/* Pixels, clipped to the screen. */
void gfx_plot(int x, int y, byte color, byte operation);
void gfx_hspan(int x1, int x2, int y, byte color, byte operation);
/* The colour of a pixel, or -1 outside the screen. */
int gfx_pixel(int x, int y);
/* Clears the screen to the background. */
void gfx_clear(void);

/* Raster operations, as the OS layer takes them. */
enum { OP_SET, OP_AND, OP_OR, OP_XOR };

#endif
