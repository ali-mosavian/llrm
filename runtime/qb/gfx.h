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
    int fg_max, bg_max;    /* the largest numbers COLOR takes, -1 for none */
    byte cga;              /* COLOR sets the CGA palette: background, palette */
    byte dac;              /* PALETTE takes a mix of red, green and blue (VGA) */
    unsigned colors;       /* attributes */
    byte bits;             /* bits of a pixel in each of its planes (GET and PUT) */
    byte planes;           /* 4 for the EGA and VGA modes with 16 colours, else 1 */
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
/* COLOR: the foreground and background, either -1 to leave it; a background
   where the mode takes none is an error. */
void gfx_set_colors(int foreground, int background);

/* A colour argument: -1 is the foreground; out of range is an error. */
byte gfx_color(int color);
byte gfx_attribute(int color);

/* Pixels, clipped to the screen. */
void gfx_plot(int x, int y, byte color, byte operation);
void gfx_hspan(int x1, int x2, int y, byte color, byte operation);
/* The colour of a pixel, or -1 outside the screen. */
int gfx_pixel(int x, int y);
/* From pixel `x` (on the screen) toward `last`, which may be off it, the first
   pixel on the screen that is of colour `c1` or `c2` (`match` 1) or of neither
   (`match` 0); -1 when there is none. */
int gfx_search(int x, int last, int y, unsigned c1, unsigned c2, int match);
/* Clears the screen to the background. */
void gfx_clear(void);

/* LINE, for the drawing that ends in lines (gline.c). */
void gfx_line_between(int color, int style, int how);

/* Raster operations, as the OS layer takes them. */
enum { OP_SET, OP_AND, OP_OR, OP_XOR };

#endif
