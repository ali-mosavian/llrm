/* The graphics screen of this target: EGA's planes at A000h, every pixel a bit
   in each of four planes, drawn through the graphics controller and the BIOS
   (gfxdev.c).  Pixels are numbered from 0, 0 at the top left; a colour is 0 to
   15, and an operation 0 to 3: set, and, or, xor. */
#ifndef QB_GFXDEV_H
#define QB_GFXDEV_H

#include "platform.h"

/* Sets the BIOS video mode, and says whether the screen is in it. */
int gd_set_mode(unsigned mode);
/* Gives palette entry `index` the adapter's colour `color`. */
void gd_palette(unsigned index, unsigned color);
/* The CGA modes' colour select: the background 0 to 15, and which palette, 0 or
   1, as the BIOS sets them. */
void gd_cga_color(unsigned background, unsigned palette);
/* Gives colour register `index` the mix of `red`, `green` and `blue`, 0 to 63. */
void gd_palette_mix(unsigned index, unsigned red, unsigned green, unsigned blue);

void gd_plot(unsigned x, unsigned y, unsigned color, unsigned operation);
/* A fill of a box of pixels: the routine for the mode, chosen with the operation by gd_fill_select (which patches the box
   loop for it, so only one fill is under way at a time), run on a box of `rows` rows of `count` pixels from `x` and row
   `y`, already clipped to the screen.  An edge byte of the box becomes old & keep ^ flip. */
typedef struct GdFill {
    void (*box)(const struct GdFill *fill, unsigned x, unsigned y, unsigned count, unsigned rows);
    void (*dot)(const struct GdFill *fill, unsigned x, unsigned y);
    void (*line)(const struct GdFill *fill, unsigned x, unsigned y, unsigned dx, unsigned dy, int step_y);
    unsigned color, operation, keep, flip;
    unsigned char tab[16];      /* CGA: the and of each place in a byte, then the xor */
} GdFill;
/* A solid line from pixel (x, y) to the right, clipped to the screen: dx pixels across and dy down (step_y 1) or up
   (-1), Bresenham's way, stepping along the longer side.  Not under gd_dots_begin. */
/* One pixel with the same fill, for primitives that have clipped to the screen: bracketed by gd_dots_begin and
   gd_dots_end, which leave the adapter as a plot would (the planar modes keep their controller's function and mask
   between the two). */
void gd_dots_begin(const GdFill *fill);
void gd_dots_end(const GdFill *fill);
void gd_fill_select(GdFill *fill, unsigned color, unsigned operation);
unsigned gd_read(unsigned x, unsigned y);
/* From pixel `x` toward `last` (either way, both on the screen), the first
   pixel that is of colour `c1` or `c2` (`match` 1) or of neither (`match` 0),
   or -1 when there is none. */
int gd_search(int x, int last, unsigned y, unsigned c1, unsigned c2, int match);
/* Copies whole rows of pixels, the first first, so that moving up over itself
   is right. */
void gd_move_rows(unsigned to, unsigned from, unsigned count);

/* A character cell: `height` rows of 8 pixels, one byte of `bits` a row, its
   set bits in `foreground`, the others in colour 0; x a multiple of 8. */
void gd_glyph(unsigned x, unsigned y, const u8 QB_FAR *bits,
              unsigned height, unsigned foreground);
/* The BIOS's font of characters `height` rows high (8, 14 or 16). */
const u8 QB_FAR *gd_font(unsigned height);

#endif
