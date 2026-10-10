#ifndef QB_DEVICE_H
#define QB_DEVICE_H
#include "platform.h"
typedef struct Regs {
    unsigned rax, rbx, rcx, rdx, rbp, res, rflags;
} Regs;
typedef struct BlockOp {
    void *dst;
    const void *src;
    unsigned count;
    unsigned value;
} BlockOp;
typedef struct FillBox {
    void *dst;
    unsigned rows, middle, steps[2], phase, edges, left, right;
} FillBox;
typedef struct FillLine {
    void *dst;
    unsigned count;
    int decision, minor4, step4, ystep;
    unsigned style, x_major, pmask, bpp, color;
} FillLine;
typedef struct FillXfer {
    void *screen;
    void *array;
    unsigned rows, sbytes, abytes, stride, shift, first, last;
    int step;
    unsigned bank;
} FillXfer;
typedef struct FillScan {
    void *at;
    unsigned count, c1, c2, flags, first, last, middle;
    int found;
    unsigned hits;
} FillScan;
void dev_scan_linear(FillScan *scan);
void dev_scan_planar(FillScan *scan);
void dev_scan_packed(FillScan *scan);
void dev_get_rows(FillXfer *xfer);
void dev_put_select(unsigned how);
void dev_put_rows(FillXfer *xfer);
void dev_put_planar(FillXfer *xfer);
void dev_line_linear(FillLine *line);
void dev_line_packed(FillLine *line);
void dev_line_planar(FillLine *line);
void dev_fill_select(unsigned operation, unsigned byte);
void dev_fill_box(FillBox *box);
void dev_int10(Regs *regs);
void dev_int10r(Regs *regs);
void dev_outw(unsigned port, unsigned word);
void dev_move(BlockOp *op);
void dev_fill(BlockOp *op);
#endif
