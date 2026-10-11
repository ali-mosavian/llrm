/* This target's devices as the QB runtime uses them (QB rt/ll*.asm): the
   standard output being the screen, the clock, the speaker, the text screen.
   C around the intrinsics of intrin.asm; a target brings its own. */
#ifndef QB_DEVICE_H
#define QB_DEVICE_H

#include "platform.h"

/* The registers of a software interrupt, in and out (intrin.asm). */
typedef struct Regs {
    unsigned rax, rbx, rcx, rdx, rbp, res, rflags;
} Regs;
typedef struct BlockOp {
    void QB_FAR *dst;
    const void QB_FAR *src;
    unsigned count;
    unsigned value;
} BlockOp;
/* A box fill (fill.asm): `rows` rows from `dst`, each a run of `middle` bytes of the operation and pattern B$FSEL last set,
   with an edge byte before and after it if `edges` (new = old & and ^ xor, `left` and `right` hold the and low, the xor
   next); a row starts steps[0] after the one before when that is even, steps[1] when odd, `phase` (in bytes) naming
   the first. */
typedef struct FillBox {
    void QB_FAR *dst;
    unsigned rows, middle, steps[2], phase, edges, left, right;
} FillBox;
/* A line (fill.asm), set with the colour B$FSEL last had: `count` pixels from `dst` (the byte holding the first), Bresenham's decision
   value and its two increments (`minor4`, `step4`), `x_major` 1 if the longer side is x, 0 if y, 2 for a vertical line; `style` the 16 bits
   of the line style, the next pixel's the high bit.  `ystep` is the bytes to the next row, down or up (CGA: 80 and -80, the banks
   toggling).  The first pixel's mask in its byte is `pmask` (a bit for the planar modes; the bits of the pixel for CGA, `bpp` wide, and
   `color` the colour in every pixel of a byte; planar: the colour). */
typedef struct FillLine {
    void QB_FAR *dst;
    unsigned count;
    int decision, minor4, step4, ystep;
    unsigned style, x_major, pmask, bpp, color;
} FillLine;
/* One plane's pass of GET or PUT over `rows` rows (fill.asm): the screen's first byte, the array's run of this plane in the first row, and
   for each row `sbytes` screen bytes against `abytes` array bytes, the bits of one starting `shift` bits into the other (a screen byte
   takes the array bit 0 at its bit `shift`); `first` and `last` are the masks of the first and last screen byte, and for GET the
   mask of the last array byte.  Row to row the array run moves `stride` bytes and the screen `step`, or by the CGA banks if `bank`. */
typedef struct FillXfer {
    void QB_FAR *screen;
    void QB_FAR *array;
    unsigned rows, sbytes, abytes, stride, shift, first, last;
    int step;
    unsigned bank;
} FillXfer;
/* A scan for the first pixel (or planar byte of pixels) that is of colour `c1` or `c2` (`flags` bit 0) or of neither (bit 0 clear), from
   `at` forwards or, `flags` bit 1, backwards, over `count` of them (fill.asm).  Planar: `first` and `last` are the masks of the pixels
   to take in the first and last byte, `middle` in the others, and for CGA (`flags` bit 2 for 2-bit pixels) `c1` and `c2` are the byte
   with the colour in every pixel and the masks have a bit for each pixel (its low bit if 2).  Out: `found` the index of the one (a byte, and
   `hits` the pixels of it), -1 if none. */
typedef struct FillScan {
    void QB_FAR *at;
    unsigned count, c1, c2, flags, first, last, middle;
    int found;
    unsigned hits;
} FillScan;
/* The eight points of a circle step for the planar or CGA modes (fill.asm): the screen's base, the offsets of the four rows (cy-v, cy+v, cy-z,
   cy+z) and of the four columns' bytes (cx+u, cx-u, cx+w, cx-w) with the mask of the pixel in each, and the colour (CGA: in every pixel
   of a byte); the points are (u,-v) (w,-z) (-w,-z) (-u,-v) (-u,v) (-w,z) (w,z) (u,v). */
typedef struct FillOct {
    void QB_FAR *base;
    unsigned row[4], col[4], mask[4], color;
} FillOct;

extern void dev_int10(Regs *regs);
/* Where protected mode has to ask for a real-mode result, the interrupt run in real mode; here it is the same. */
#define dev_int10r dev_int10
extern void dev_int17(Regs *regs);
extern void dev_int21(Regs *regs);
extern void dev_outb(unsigned port, unsigned value);
extern void dev_outw(unsigned port, unsigned word);
extern unsigned dev_inb(unsigned port);
extern void dev_interrupts_off(void);
extern void dev_interrupts_on(void);
extern void dev_move(BlockOp *op);
extern void dev_fill(BlockOp *op);
extern void dev_fill_select(unsigned operation, unsigned byte);
extern void dev_fill_box(FillBox *box);
extern void dev_line_linear(FillLine *line);
extern void dev_line_packed(FillLine *line);
extern void dev_line_planar(FillLine *line);
extern void dev_get_rows(FillXfer *xfer);
extern void dev_scan_linear(FillScan *scan);
extern void dev_scan_planar(FillScan *scan);
extern void dev_scan_packed(FillScan *scan);
extern void dev_octet_planar(FillOct *oct);
extern void dev_octet_packed(FillOct *oct);
extern void dev_put_select(unsigned how);
extern void dev_put_rows(FillXfer *xfer);
extern void dev_put_planar(FillXfer *xfer);
extern void dev_atan2(const double *y, const double *x, double *out);
extern void dev_sincos(const double *angle, double *sine, double *cosine);

#pragma aux dev_int10 "B$INT10"
#pragma aux dev_int17 "B$INT17"
#pragma aux dev_int21 "B$INT21"
#pragma aux dev_outb "B$OUTB"
#pragma aux dev_outw "B$OUTW"
#pragma aux dev_inb "B$INB"
#pragma aux dev_interrupts_off "B$CLI"
#pragma aux dev_interrupts_on "B$STI"
#pragma aux dev_move "B$MOVE"
#pragma aux dev_fill "B$FILL"
#pragma aux dev_fill_select "B$FSEL"
#pragma aux dev_fill_box "B$FBOX"
#pragma aux dev_line_linear "B$FLIN"
#pragma aux dev_line_packed "B$FLIC"
#pragma aux dev_line_planar "B$FLIP"
#pragma aux dev_get_rows "B$FGET"
#pragma aux dev_scan_linear "B$FSCL"
#pragma aux dev_scan_planar "B$FSCP"
#pragma aux dev_scan_packed "B$FSCC"
#pragma aux dev_octet_planar "B$FOCP"
#pragma aux dev_octet_packed "B$FOCC"
#pragma aux dev_put_select "B$FPSEL"
#pragma aux dev_put_rows "B$FPUC"
#pragma aux dev_put_planar "B$FPUP"
#pragma aux dev_atan2 "B$ATAN2"
#pragma aux dev_sincos "B$SINCOS"

/* Whether standard output is the screen and not a file or a pipe. */
int dev_stdout_is_screen(void);
/* Whether the printer is there and ready. */
int dev_printer_ready(void);
/* The time of day in hundredths of a second since midnight. */
long dev_clock(void);
/* From now on the clock's tick (18.2 a second) calls music_tick (play.c); the
   handler is put back when the program ends.  Called once. */
void dev_ticker_start(void);
#pragma aux dev_ticker_start "B$TICKON"
/* A tone of `hertz` until the next call; 0 is silence. */
void dev_tone(unsigned hertz);

/* The text screen: cells of a character and an attribute from 0, 0 at the top
   left. */
unsigned dev_text_size(void);        /* rows in the high byte, columns in the low */
unsigned dev_text_cursor(void);      /* row in the high byte */
void dev_text_move(unsigned row, unsigned column);
void dev_text_cursor_show(int visible);
void dev_text_put(unsigned row, unsigned column, unsigned character,
                  unsigned attribute);
void dev_text_write(unsigned row, unsigned column, const char *text,
                    unsigned count, unsigned attribute);
/* The rows `top` to `bottom` move up `lines`, the rows freed are blanks of
   `attribute`; no lines, or as many as the rows, clears them. */
void dev_text_scroll(unsigned top, unsigned bottom, unsigned lines,
                     unsigned attribute);

#endif
