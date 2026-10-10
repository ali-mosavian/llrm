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

extern void dev_int10(Regs *regs);
extern void dev_int17(Regs *regs);
extern void dev_int21(Regs *regs);
extern void dev_outb(unsigned port, unsigned value);
extern void dev_outw(unsigned port, unsigned word);
extern unsigned dev_inb(unsigned port);
extern void dev_move(BlockOp *op);
extern void dev_fill(BlockOp *op);
extern void dev_atan2(const double *y, const double *x, double *out);
extern void dev_sincos(const double *angle, double *sine, double *cosine);

#pragma aux dev_int10 "B$INT10"
#pragma aux dev_int17 "B$INT17"
#pragma aux dev_int21 "B$INT21"
#pragma aux dev_outb "B$OUTB"
#pragma aux dev_outw "B$OUTW"
#pragma aux dev_inb "B$INB"
#pragma aux dev_move "B$MOVE"
#pragma aux dev_fill "B$FILL"
#pragma aux dev_atan2 "B$ATAN2"
#pragma aux dev_sincos "B$SINCOS"

/* Whether standard output is the screen and not a file or a pipe. */
int dev_stdout_is_screen(void);
/* Whether the printer is there and ready. */
int dev_printer_ready(void);
/* The time of day in hundredths of a second since midnight. */
long dev_clock(void);
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
