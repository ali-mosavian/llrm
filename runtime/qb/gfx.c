/* The graphics screen: modes, colours, pixels and points (QB rt/gr*.asm). */
#include "cndriver.h"
#include "gfx.h"
#include "gfxdev.h"
#include "rtinit.h"

enum { DEFAULT_FOREGROUND = 15, TEXT_MODE = 3 };

static const GfxMode modes[] = {
    /* number, BIOS mode, size, text rows, aspect, COLOR's limits, CGA, DAC,
       attributes, bits a pixel, planes */
    {1, 0x04, 320, 200, 8, 5.0 / 6.0, 255, 255, 1, 0, 4, 2, 1},
    {2, 0x06, 640, 200, 8, 5.0 / 12.0, -1, -1, 0, 0, 2, 1, 1},
    {7, 0x0D, 320, 200, 8, 5.0 / 6.0, 15, 15, 0, 0, 16, 1, 4},
    {8, 0x0E, 640, 200, 8, 5.0 / 12.0, 15, 15, 0, 0, 16, 1, 4},
    {9, 0x10, 640, 350, 14, 35.0 / 48.0, 15, 63, 0, 0, 16, 1, 4},
    {12, 0x12, 640, 480, 16, 1.0, 15, -1, 0, 1, 16, 1, 4},
    {13, 0x13, 320, 200, 8, 5.0 / 6.0, 255, -1, 0, 1, 256, 8, 1}
};

const GfxMode *gfx_current;
byte gfx_foreground = DEFAULT_FOREGROUND, gfx_background;
static byte cga_background, cga_palette = 1;
int gfx_x1, gfx_y1, gfx_x2, gfx_y2;

/* The adapter's own colour numbers for the attributes 0 to 15 at the start of a
   mode: 0-5 and 7 as they are, 6 brown (20), 8-15 the bright ones (56-63). */
static const byte default_palette[16] = {
    0, 1, 2, 3, 4, 5, 20, 7, 56, 57, 58, 59, 60, 61, 62, 63
};

void gfx_set_background(byte color)
{
    gfx_background = color;
    gd_palette(0, color < 16 ? default_palette[color] : color);
}

void gfx_clear(void)
{
    unsigned y;

    for (y = 0; y < gfx_current->height; y++)
        gd_span(0, gfx_current->width, y, 0, OP_SET);
}

void gfx_screen(int mode)
{
    unsigned at;

    if (mode == 0) {
        if (gfx_current) {
            gd_set_mode(TEXT_MODE);
            gfx_current = 0;
            cn_graphics = 0;
            cn_mode_changed();
        }
        return;
    }
    for (at = 0; at < sizeof modes / sizeof modes[0]; at++) {
        if (modes[at].number == mode) {
            if (!gd_set_mode(modes[at].bios))
                break;
            gfx_current = &modes[at];
            gfx_foreground = modes[at].colors < 16 ? modes[at].colors - 1 : DEFAULT_FOREGROUND;
            gfx_background = 0;
            cga_background = 0;
            cga_palette = 1;
            cn_graphics = 1;
            cn_mode_changed();
            return;
        }
    }
    qb_error(BE_ILLFUN);
}

/* COLOR: the numbers a mode takes are its own, and 255 for the background of the
   EGA modes is taken and means none. */
void gfx_set_colors(int foreground, int background)
{
    const GfxMode *mode = gfx_current;
    int none = background == 255 && !mode->cga && mode->bg_max > 0;

    if (foreground > mode->fg_max || (foreground >= 0 && mode->fg_max < 0)
        || (!none && (background > mode->bg_max || (background >= 0 && mode->bg_max < 0))))
        qb_error(BE_ILLFUN);
    if (mode->cga) {
        if (foreground >= 0)
            cga_background = foreground & 15;
        if (background >= 0)
            cga_palette = background & 1;
        gd_cga_color(cga_background, cga_palette);
        return;
    }
    if (foreground >= 0)
        gfx_foreground = foreground;
    if (background >= 0 && !none)
        gfx_set_background((byte)background);
}

/* A colour number: -1 is the foreground, and one that is no attribute of the mode
   is the foreground too (the number is taken as a byte first). */
byte gfx_color(int color)
{
    if (color == -1)
        return gfx_foreground;
    return gfx_attribute(color);
}

/* The attribute a colour number means, whatever the number. */
byte gfx_attribute(int color)
{
    unsigned attribute = (unsigned)color & 0xFF;

    return attribute < gfx_current->colors ? (byte)attribute : gfx_foreground;
}

void gfx_plot(int x, int y, byte color, byte operation)
{
    if (x >= 0 && y >= 0 && (unsigned)x < gfx_current->width
        && (unsigned)y < gfx_current->height)
        gd_plot(x, y, color, operation);
}

void gfx_hspan(int x1, int x2, int y, byte color, byte operation)
{
    int last = gfx_current->width - 1;

    if (x1 > x2) {
        int swap = x1;

        x1 = x2;
        x2 = swap;
    }
    if (y < 0 || (unsigned)y >= gfx_current->height || x2 < 0 || x1 > last)
        return;
    if (x1 < 0)
        x1 = 0;
    if (x2 > last)
        x2 = last;
    gd_span(x1, x2 - x1 + 1, y, color, operation);
}

int gfx_pixel(int x, int y)
{
    if (x < 0 || y < 0 || (unsigned)x >= gfx_current->width
        || (unsigned)y >= gfx_current->height)
        return -1;
    return gd_read(x, y);
}

int gfx_search(int x, int last, int y, unsigned c1, unsigned c2, int match)
{
    int edge = (int)gfx_current->width - 1;

    if (last < 0)
        last = 0;
    else if (last > edge)
        last = edge;
    return gd_search(x, last, y, c1, c2, match);
}

/* A coordinate given as a SINGLE, rounded to the nearest pixel, a half going to
   the even one as CINT does. */
static int rounded(float v)
{
    int whole = (int)v;
    float part = v - whole;

    if (part >= 0.5f)
        whole += part == 0.5f && !(whole & 1) ? 0 : 1;
    else if (part <= -0.5f)
        whole -= part == -0.5f && !(whole & 1) ? 0 : 1;
    return whole;
}

static void need_graphics(void)
{
    if (!gfx_current)
        qb_error(BE_ILLFUN);
}

void B_N1I2(int x, int y)
{
    need_graphics();
    gfx_x1 = x;
    gfx_y1 = y;
}

void B_N1R4(float x, float y)
{
    need_graphics();
    gfx_x1 = rounded(x);
    gfx_y1 = rounded(y);
}

void B_N2I2(int x, int y)
{
    gfx_x2 = x;
    gfx_y2 = y;
}

void B_N2R4(float x, float y)
{
    gfx_x2 = rounded(x);
    gfx_y2 = rounded(y);
}

/* B$PSTC: PSET and PRESET at the first point. */
void B_PSTC(int color)
{
    gfx_plot(gfx_x1, gfx_y1, gfx_attribute(color), OP_SET);
}

/* B$PSET and B$PRST: PSET and PRESET without a colour, which take the
   foreground and the background; the background is colour 0 made to look like
   the background colour (gfx_set_background), so PRESET draws 0. */
void B_PSET(void)
{
    gfx_plot(gfx_x1, gfx_y1, gfx_foreground, OP_SET);
}

void B_PRST(void)
{
    gfx_plot(gfx_x1, gfx_y1, 0, OP_SET);
}

/* B$PNR4: POINT(x, y), the colour there or -1. */
int B_PNR4(float x, float y)
{
    need_graphics();
    return gfx_pixel(rounded(x), rounded(y));
}

/* B$PNI2: POINT with whole-number coordinates. */
int B_PNI2(int x, int y)
{
    need_graphics();
    return gfx_pixel(x, y);
}

/* B$PAL2: PALETTE attribute, colour. */
void B_PAL2(int attribute, long color)
{
    need_graphics();
    if (attribute < 0 || (unsigned)attribute >= gfx_current->colors || color < 0)
        qb_error(BE_ILLFUN);
    if (gfx_current->dac) {
        /* red, green and blue, 0 to 63 each, in the three bytes from the low one */
        if ((color & 0xFF) > 63 || (color >> 8 & 0xFF) > 63 || (color >> 16) > 63)
            qb_error(BE_ILLFUN);
        gd_palette_mix(attribute, color & 0xFF, color >> 8 & 0xFF, (unsigned)(color >> 16));
    } else {
        if (color > 63)
            qb_error(BE_ILLFUN);
        gd_palette(attribute, (byte)color);
    }
}
#pragma aux B_N1I2 "B$N1I2"
#pragma aux B_N1R4 "B$N1R4"
#pragma aux B_N2I2 "B$N2I2"
#pragma aux B_N2R4 "B$N2R4"
#pragma aux B_PSTC "B$PSTC"
#pragma aux B_PSET "B$PSET"
#pragma aux B_PRST "B$PRST"
#pragma aux B_PNR4 "B$PNR4"
#pragma aux B_PNI2 "B$PNI2"
#pragma aux B_PAL2 "B$PAL2"

/* SCREEN is taken here once a program has this linked (screen.c). */
extern void (*screen_set_mode)(int mode);
extern void (*screen_set_colors)(int foreground, int background);

/* The graphics screen's text driver goes with it. */
extern void cn_gfx_xinit(void);
void (*const gfx_needs_text)(void) = cn_gfx_xinit;

/* The program ends in the text mode it began in. */
static void gfx_term(void)
{
    gfx_screen(0);
}

static Comp gfx_comp = { 0, C_GR, { 0, 0, 0, 0, 0, gfx_term } };

#define XI_FN gfx_xinit
#include "xi.h"
void gfx_xinit(void)
{
    screen_set_mode = gfx_screen;
    screen_set_colors = gfx_set_colors;
    qb_comp_add(&gfx_comp);
}

/* What the compiler names to have the graphics screen linked: a program that
   draws or sets a mode refers to these, and they are only to be found. */
const byte graphics_used, ega_used, vga_used, cga_used, mono_used;
#pragma aux graphics_used "B$GRPUSED"
#pragma aux ega_used "B$EGAUSED"
#pragma aux vga_used "B$VGAUSED"
#pragma aux cga_used "B$CGAUSED"
#pragma aux mono_used "B$MONUSED"
