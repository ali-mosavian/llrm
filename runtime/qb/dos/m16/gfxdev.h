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

void gd_plot(unsigned x, unsigned y, unsigned color, unsigned operation);
void gd_span(unsigned x, unsigned count, unsigned y, unsigned color,
             unsigned operation);
unsigned gd_read(unsigned x, unsigned y);
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
