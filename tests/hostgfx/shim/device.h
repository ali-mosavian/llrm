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
void dev_fill_select(unsigned operation, unsigned byte);
void dev_fill_box(FillBox *box);
void dev_int10(Regs *regs);
void dev_int10r(Regs *regs);
void dev_outw(unsigned port, unsigned word);
void dev_move(BlockOp *op);
void dev_fill(BlockOp *op);
#endif
