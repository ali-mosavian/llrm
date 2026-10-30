/* The graphics screen: modes, colours, pixels and points (QB rt/gr*.asm). */
#include "ad.h"
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
    {13, 0x13, 320, 200, 8, 5.0 / 6.0, 255, -1, 0, 1, 256, 8, 1}};

const GfxMode *gfx_current;
byte gfx_foreground = DEFAULT_FOREGROUND, gfx_background;
static byte cga_background, cga_palette = 1;
int gfx_x1, gfx_y1, gfx_x2, gfx_y2;
int gfx_window_on;
int gfx_view_left, gfx_view_top, gfx_view_right, gfx_view_bottom, gfx_origin_x, gfx_origin_y;
/* The last referenced point: STEP is taken from it, and LINE -(x, y) starts at
 * it. */
int gfx_last_x, gfx_last_y;

/* The adapter's own colour numbers for the attributes 0 to 15 at the start of a
   mode: 0-5 and 7 as they are, 6 brown (20), 8-15 the bright ones (56-63). */
static const byte default_palette[16] = {0, 1, 2, 3, 4, 5, 20, 7, 56, 57, 58, 59, 60, 61, 62, 63};

void gfx_set_background(byte color)
{
    gfx_background = color;
    gd_palette(0, color < 16 ? default_palette[color] : color);
}

/* The whole screen is the viewport, with the origin in its corner. */
static int view_screen;
static void window_recompute(void);

static void view_reset(void)
{
    view_screen = 0;
    gfx_view_left = gfx_view_top = gfx_origin_x = gfx_origin_y = 0;
    gfx_view_right = (int)gfx_current->width - 1;
    gfx_view_bottom = (int)gfx_current->height - 1;
}

/* The viewport is cleared, as CLS does. */
void gfx_clear(void)
{
    GdFill fill;

    gd_fill_select(&fill, 0, OP_SET);
    fill.box(&fill, (unsigned)gfx_view_left, (unsigned)gfx_view_top, (unsigned)(gfx_view_right - gfx_view_left + 1),
             (unsigned)(gfx_view_bottom - gfx_view_top + 1));
}

static int active_page, visible_page;

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
            if (gfx_current != &modes[at]) {
                gfx_last_x = (int)modes[at].width / 2;
                gfx_last_y = (int)modes[at].height / 2;
            }
            gfx_current = &modes[at];
            active_page = visible_page = 0;
            gfx_window_on = 0;
            view_reset();
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

/* COLOR: the numbers a mode takes are its own, and 255 for the background of
   the EGA modes is taken and means none. */
void gfx_set_colors(int foreground, int background)
{
    const GfxMode *mode = gfx_current;
    int none = background == 255 && !mode->cga && mode->bg_max > 0;

    if (foreground > mode->fg_max || (foreground >= 0 && mode->fg_max < 0) ||
        (!none && (background > mode->bg_max || (background >= 0 && mode->bg_max < 0))))
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

/* A colour number: -1 is the foreground, and one that is no attribute of the
   mode is the foreground too (the number is taken as a byte first). */
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
    if (x >= gfx_view_left && y >= gfx_view_top && x <= gfx_view_right && y <= gfx_view_bottom)
        gd_plot(x, y, color, operation);
}

int gfx_clip_box(int *x1, int *y1, int *x2, int *y2)
{
    int right = gfx_view_right, bottom = gfx_view_bottom;

    if (*x1 > *x2) {
        int swap = *x1;

        *x1 = *x2;
        *x2 = swap;
    }
    if (*y1 > *y2) {
        int swap = *y1;

        *y1 = *y2;
        *y2 = swap;
    }
    if (*x2 < gfx_view_left || *y2 < gfx_view_top || *x1 > right || *y1 > bottom)
        return 0;
    if (*x1 < gfx_view_left)
        *x1 = gfx_view_left;
    if (*y1 < gfx_view_top)
        *y1 = gfx_view_top;
    if (*x2 > right)
        *x2 = right;
    if (*y2 > bottom)
        *y2 = bottom;
    return 1;
}

int gfx_pixel(int x, int y)
{
    if (x < gfx_view_left || y < gfx_view_top || x > gfx_view_right || y > gfx_view_bottom)
        return -1;
    return gd_read(x, y);
}

int gfx_search(int x, int last, int y, unsigned c1, unsigned c2, int match)
{
    int edge = gfx_view_right;

    if (last < gfx_view_left)
        last = gfx_view_left;
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

/* WINDOW (QB rt/grwindow.asm, grfpinit.asm): the logical rectangle, its ratios to the viewport's size in pixels (the
   whole screen's for VIEW SCREEN), and the last point as reals, with the pixel it was mapped to. */
static int window_screen;
static double wxmin, wymin, wxmax, wymax, lxdif, lydif, pxdif, pydif;
static float logical_x, logical_y;
static int logical_at_x, logical_at_y;

/* (v - offset) * scale rounded to a whole number as the FPU rounds it: the two operations in its wider format, then to
   the nearest, a half to the even. A double would round the product to a half where the wider value lies just under
   it, so the product is kept as a double and the error it lost. */
static int scaled_round(double v, double offset, double scale)
{
    static const double split = 134217729.0;
    double d, bb, derr, p, t, ah, al, bh, bl, lo, part;
    long whole;

    d = v - offset;
    bb = d - v;
    derr = (v - (d - bb)) + (-offset - bb);
    p = d * scale;
    t = split * d;
    ah = t - (t - d);
    al = d - ah;
    t = split * scale;
    bh = t - (t - scale);
    bl = scale - bh;
    lo = ((ah * bh - p) + ah * bl + al * bh) + al * bl + derr * scale;
    whole = (long)p;
    part = (p - whole) + lo;
    if (part >= 0.5)
        whole += part == 0.5 && !(whole & 1) ? 0 : 1;
    else if (part <= -0.5)
        whole -= part == -0.5 && !(whole & 1) ? 0 : 1;
    return (int)whole;
}

static int map_x(float x)
{
    return scaled_round(x, wxmin, lxdif) + gfx_origin_x;
}

static int map_y(float y)
{
    if (window_screen)
        return scaled_round(y, wymin, lydif) + gfx_origin_y;
    return -scaled_round(y, wymax, lydif) + gfx_origin_y;
}

/* The logical point a pixel is. */
static void logical_of_last(void)
{
    double x = (gfx_last_x - gfx_origin_x) * pxdif, y = (gfx_last_y - gfx_origin_y) * pydif;

    logical_x = (float)(x + wxmin);
    logical_y = (float)(window_screen ? y + wymin : -y + wymax);
    logical_at_x = gfx_last_x;
    logical_at_y = gfx_last_y;
}

/* The ratios after the window or the viewport changed; the last point keeps its pixel. */
static void window_recompute(void)
{
    double vx = view_screen ? (double)gfx_current->width - 1 : (double)(gfx_view_right - gfx_view_left);
    double vy = view_screen ? (double)gfx_current->height - 1 : (double)(gfx_view_bottom - gfx_view_top);

    lxdif = vx / (wxmax - wxmin);
    pxdif = 1.0 / lxdif;
    lydif = vy / (wymax - wymin);
    pydif = 1.0 / lydif;
    logical_of_last();
}

double gfx_window_scale(void)
{
    return lxdif;
}

/* The first and second points of a statement, absolute (B$N..) or (B$S..) a
   step from the last point and from the first; each is the last point after. A
   step is rounded before it is added, as BC does. */
static void first_point(int x, int y)
{
    need_graphics();
    gfx_last_x = gfx_x1 = x;
    gfx_last_y = gfx_y1 = y;
}

static void second_point(int x, int y)
{
    gfx_last_x = gfx_x2 = x;
    gfx_last_y = gfx_y2 = y;
}

/* A point while a window is active: the logical last point moves by a step or goes to the point, and is mapped. */
static void window_point(int second, int step, float x, float y)
{
    need_graphics();
    if (gfx_last_x != logical_at_x || gfx_last_y != logical_at_y)
        logical_of_last();
    if (step) {
        logical_x = (float)((double)logical_x + x);
        logical_y = (float)((double)logical_y + y);
    } else {
        logical_x = x;
        logical_y = y;
    }
    if (second)
        second_point(map_x(logical_x), map_y(logical_y));
    else
        first_point(map_x(logical_x), map_y(logical_y));
    logical_at_x = gfx_last_x;
    logical_at_y = gfx_last_y;
}

#pragma aux B_N1I2 "B$N1I2"
#pragma aux B_N1R4 "B$N1R4"
#pragma aux B_N2I2 "B$N2I2"
#pragma aux B_N2R4 "B$N2R4"
#pragma aux B_S1I2 "B$S1I2"
#pragma aux B_S1R4 "B$S1R4"
#pragma aux B_S2I2 "B$S2I2"
#pragma aux B_S2R4 "B$S2R4"
#pragma aux B_VIEW "B$VIEW"
#pragma aux B_WIND "B$WIND"
#pragma aux B_WIN0 "B$WIN0"
#pragma aux B_VEW0 "B$VEW0"
#pragma aux B_PSTC "B$PSTC"
#pragma aux B_PSET "B$PSET"
#pragma aux B_PRST "B$PRST"
#pragma aux B_PMAP "B$PMAP"
#pragma aux B_PNR4 "B$PNR4"
#pragma aux B_PNI2 "B$PNI2"
#pragma aux B_PAL2 "B$PAL2"
#pragma aux B_PALU "B$PALU"
#pragma aux graphics_used "B$GRPUSED"
#pragma aux ega_used "B$EGAUSED"
#pragma aux vga_used "B$VGAUSED"
#pragma aux cga_used "B$CGAUSED"
#pragma aux mono_used "B$MONUSED"

void B_N1I2(short x, short y)
{
    if (gfx_window_on)
        window_point(0, 0, x, y);
    else
        first_point(x + gfx_origin_x, y + gfx_origin_y);
}

void B_N1R4(float x, float y)
{
    if (gfx_window_on)
        window_point(0, 0, x, y);
    else
        first_point(rounded(x) + gfx_origin_x, rounded(y) + gfx_origin_y);
}

void B_N2I2(short x, short y)
{
    if (gfx_window_on)
        window_point(1, 0, x, y);
    else
        second_point(x + gfx_origin_x, y + gfx_origin_y);
}

void B_N2R4(float x, float y)
{
    if (gfx_window_on)
        window_point(1, 0, x, y);
    else
        second_point(rounded(x) + gfx_origin_x, rounded(y) + gfx_origin_y);
}

void B_S1I2(short x, short y)
{
    if (gfx_window_on)
        window_point(0, 1, x, y);
    else
        first_point(gfx_last_x + x, gfx_last_y + y);
}

void B_S1R4(float x, float y)
{
    if (gfx_window_on)
        window_point(0, 1, x, y);
    else
        first_point(gfx_last_x + rounded(x), gfx_last_y + rounded(y));
}

void B_S2I2(short x, short y)
{
    if (gfx_window_on)
        window_point(1, 1, x, y);
    else
        second_point(gfx_x1 + x, gfx_y1 + y);
}

void B_S2R4(float x, float y)
{
    if (gfx_window_on)
        window_point(1, 1, x, y);
    else
        second_point(gfx_x1 + rounded(x), gfx_y1 + rounded(y));
}

/* B$PSTC: PSET and PRESET at the first point. */
void B_PSTC(short color)
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
    if (gfx_window_on)
        return gfx_pixel(map_x(x), map_y(y));
    return gfx_pixel(rounded(x) + gfx_origin_x, rounded(y) + gfx_origin_y);
}

/* B$PNI2: POINT with whole-number coordinates. */
int B_PNI2(short x, short y)
{
    need_graphics();
    if (gfx_window_on)
        return gfx_pixel(map_x(x), map_y(y));
    return gfx_pixel(x + gfx_origin_x, y + gfx_origin_y);
}

/* B$VEW0: VIEW with nothing after it: the whole screen again, the cursor at its centre. */
void B_VEW0(void)
{
    need_graphics();
    view_reset();
    gfx_last_x = (int)gfx_current->width / 2;
    gfx_last_y = (int)gfx_current->height / 2;
    if (gfx_window_on)
        window_recompute();
}

/* B$VIEW: VIEW [SCREEN] (x1, y1)-(x2, y2), fill, border; -1 for a colour left out. The corners are put in order
   and must differ and lie on the screen. An ordinary VIEW moves the origin to the corner; the fill covers the
   viewport, the border is the box one pixel outside it, and the cursor goes to the viewport's centre. */
void B_VIEW(short x1, short y1, short x2, short y2, short fill, short border, short screen)
{
    int left, top, right, bottom;

    need_graphics();
    left = x1 < x2 ? x1 : x2;
    right = x1 < x2 ? x2 : x1;
    top = y1 < y2 ? y1 : y2;
    bottom = y1 < y2 ? y2 : y1;
    if (x1 == x2 || y1 == y2 || left < 0 || top < 0 || right >= (int)gfx_current->width ||
        bottom >= (int)gfx_current->height)
        qb_error(BE_ILLFUN);
    view_reset();
    gfx_view_left = left;
    gfx_view_top = top;
    gfx_view_right = right;
    gfx_view_bottom = bottom;
    view_screen = screen != 0;
    if (!screen) {
        gfx_origin_x = left;
        gfx_origin_y = top;
    }
    if (fill != -1) {
        GdFill painter;

        gd_fill_select(&painter, gfx_attribute(fill), OP_SET);
        painter.box(&painter, (unsigned)left, (unsigned)top, (unsigned)(right - left + 1), (unsigned)(bottom - top + 1));
    }
    if (border != -1) {
        int keep_left = gfx_view_left, keep_top = gfx_view_top, keep_right = gfx_view_right,
            keep_bottom = gfx_view_bottom;

        gfx_view_left = gfx_view_top = 0;
        gfx_view_right = (int)gfx_current->width - 1;
        gfx_view_bottom = (int)gfx_current->height - 1;
        gfx_x1 = left - 1;
        gfx_y1 = top - 1;
        gfx_x2 = right + 1;
        gfx_y2 = bottom + 1;
        gfx_line_between(border, -1, 1);
        gfx_view_left = keep_left;
        gfx_view_top = keep_top;
        gfx_view_right = keep_right;
        gfx_view_bottom = keep_bottom;
    }
    gfx_last_x = left + (right - left + 1) / 2;
    gfx_last_y = top + (bottom - top + 1) / 2;
    if (gfx_window_on)
        window_recompute();
}

/* B$WIN0: WINDOW with nothing after it: the window is off. */
void B_WIN0(void)
{
    need_graphics();
    gfx_window_on = 0;
}

/* B$WIND: WINDOW [SCREEN] (x1, y1)-(x2, y2). The corners are put in order and must differ. Unless SCREEN, y grows
   upward. */
void B_WIND(float x1, float y1, float x2, float y2, short screen)
{
    gfx_window_on = 0;
    need_graphics();
    wxmin = x1 < x2 ? x1 : x2;
    wxmax = x1 < x2 ? x2 : x1;
    wymin = y1 < y2 ? y1 : y2;
    wymax = y1 < y2 ? y2 : y1;
    if (x1 == x2 || y1 == y2)
        qb_error(BE_ILLFUN);
    window_screen = screen != 0;
    gfx_window_on = 1;
    window_recompute();
}

/* Whether `color` is one the palette takes: in a DAC mode red, green and blue,
   0 to 63 each, in the three bytes from the low one; otherwise 0 to 63. */
static int palette_valid(long color)
{
    if (color < 0)
        return 0;
    if (gfx_current->dac)
        return (color & 0xFF) <= 63 && (color >> 8 & 0xFF) <= 63 && (color >> 16) <= 63;
    return color <= 63;
}

static void palette_set(short attribute, long color)
{
    if (gfx_current->dac)
        gd_palette_mix(attribute, color & 0xFF, color >> 8 & 0xFF, (unsigned)(color >> 16));
    else
        gd_palette(attribute, (byte)color);
}

/* B$PAL2: PALETTE attribute, colour. */
void B_PAL2(short attribute, long color)
{
    need_graphics();
    if (attribute < 0 || (unsigned)attribute >= gfx_current->colors || !palette_valid(color))
        qb_error(BE_ILLFUN);
    palette_set(attribute, color);
}

/* B$PALU: PALETTE USING array(i) (QB rt/gwpal.asm B$PALU, llxgasup.asm
   B$EgaPalSet).  Attribute n takes element n of the array from the one named
   to its end: a LONG array in a DAC mode, where an INTEGER one is an illegal
   function call, and either elsewhere.  The elements counted are the bytes
   left halved, or quartered for LONG ones, at most 65535, and fewer than the
   screen has colours is an illegal function call.  An element of -1 leaves its
   attribute alone; all are checked before any is set. */
void B_PALU(qb_data_ptr data, const AD *ad)
{
    unsigned long bytes;
    unsigned count, at;
    const int wide = ad->elem == 4;
    const short QB_FAR *words = (const short QB_FAR *)data;
    const long QB_FAR *longs = (const long QB_FAR *)data;

    need_graphics();
    if (gfx_current->dac && !wide)
        qb_error(BE_ILLFUN);
    bytes = ad_remaining(ad, data) >> (wide ? 2 : 1);
    count = bytes > 65535UL ? 65535U : (unsigned)bytes;
    if (count < gfx_current->colors)
        qb_error(BE_ILLFUN);
    for (at = 0; at < gfx_current->colors; at++) {
        long color = wide ? longs[at] : words[at];

        if (color != -1 && !palette_valid(color))
            qb_error(BE_ILLFUN);
    }
    for (at = 0; at < gfx_current->colors; at++) {
        long color = wide ? longs[at] : words[at];

        if (color != -1)
            palette_set(at, color);
    }
}

/* B$PMAP: PMAP(expr, funct) -- 0 and 1 map a logical x or y to the viewport's pixel, 2 and 3 a pixel to the logical
   point; the pixel is counted from the viewport's corner. With no window the points are the pixels. */
float *B_PMAP(float expr, short funct)
{
    static float result;
    int pixel;

    need_graphics();
    if (funct < 0 || funct > 3)
        qb_error(BE_ILLFUN);
    pixel = rounded(expr);
    if (!gfx_window_on)
        result = (float)(funct < 2 ? scaled_round(expr, 0.0, 1.0) : pixel);
    else if (funct == 0)
        result = (float)scaled_round(expr, wxmin, lxdif);
    else if (funct == 1)
        result = (float)(window_screen ? scaled_round(expr, wymin, lydif) : -scaled_round(expr, wymax, lydif));
    else if (funct == 2)
        result = (float)(pixel * pxdif + wxmin);
    else
        result = (float)(window_screen ? pixel * pydif + wymin : -(pixel * pydif) + wymax);
    return &result;
}


/* SCREEN is taken here once a program has this linked (screen.c). */
extern void (*screen_set_mode)(int mode);
extern int (*screen_set_pages)(int active, int visible);
extern void (*screen_set_colors)(int foreground, int background);

/* The graphics screen's text driver goes with it. */
extern void cn_gfx_xinit(void);
void (*const gfx_needs_text)(void) = cn_gfx_xinit;

/* SCREEN's pages: -1 keeps the one in use. The text screen has page 0 only here. */
static int gfx_set_pages(int active, int visible)
{
    if (active < 0)
        active = active_page;
    if (visible < 0)
        visible = visible_page;
    if (!gfx_current)
        return active == 0 && visible == 0;
    if (!gd_set_pages((unsigned)active, (unsigned)visible))
        return 0;
    active_page = active;
    visible_page = visible;
    return 1;
}

/* The program ends in the text mode it began in. */
static void gfx_term(void)
{
    gfx_screen(0);
}

static Comp gfx_comp = {0, C_GR, {0, 0, 0, 0, 0, gfx_term}};

#define XI_FN gfx_xinit
#include "xi.h"

void gfx_xinit(void)
{
    screen_set_mode = gfx_screen;
    screen_set_colors = gfx_set_colors;
    screen_set_pages = gfx_set_pages;
    qb_comp_add(&gfx_comp);
}

/* What the compiler names to have the graphics screen linked: a program that
   draws or sets a mode refers to these, and they are only to be found. */
const byte graphics_used, ega_used, vga_used, cga_used, mono_used;
