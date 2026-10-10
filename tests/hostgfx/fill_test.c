/* The box fills of runtime/qb/dos/gfxdev.c against the pixel plot, on the host: for the 256-colour and the two CGA modes, every
   operation, random colours, rows and spans over random video memory.  Exits 0, or 1 with the case that differed.  (The planar
   modes need a model of the graphics controller: the screens of tests/qbrt/gfx_*.bas, against BCOM45's, are theirs.) */
#include <stdio.h>
#include <stdlib.h>
#include "../../runtime/qb/dos/gfxdev.c"

unsigned char vga_ram[0x10000], cga_ram[0x4000];
static unsigned current_mode;

void dev_int10(Regs *r)
{
    if (r->rax == 0x0F00)
        r->rax = current_mode;
    else
        current_mode = r->rax;
}
void dev_int10r(Regs *r) { dev_int10(r); }
void dev_outw(unsigned port, unsigned word) { (void)port; (void)word; }
void dev_move(BlockOp *op) { memmove(op->dst, op->src, op->count); }
void dev_fill(BlockOp *op)
{
    unsigned short *at = op->dst;

    for (unsigned n = 0; n < op->count; n++)
        *at++ = (unsigned short)op->value;
}

static unsigned rng = 7;
static unsigned next(void) { rng = rng * 1103515245u + 12345u; return (rng >> 8) & 0xFFFFFF; }

int main(void)
{
    static const struct { unsigned mode, width, colors; } modes[] = { { 0x13, 320, 256 }, { 4, 320, 4 }, { 6, 640, 2 } };

    for (unsigned m = 0; m < 3; m++) {
        gd_set_mode(modes[m].mode);
        for (unsigned round = 0; round < 6000; round++) {
            unsigned x = next() % modes[m].width, y = next() % 200, color = next() % modes[m].colors, op = next() % 4;
            unsigned count = 1 + next() % (modes[m].width - x), rows = 1 + next() % (200 - y);
            static unsigned char before[0x10000], after[0x10000];
            unsigned char *ram = modes[m].mode == 0x13 ? vga_ram : cga_ram;
            unsigned size = modes[m].mode == 0x13 ? 64000 : 16384;
            GdFill fill;

            for (unsigned i = 0; i < size; i++)
                ram[i] = (unsigned char)next();
            memcpy(before, ram, size);
            for (unsigned r = 0; r < rows; r++)
                for (unsigned i = 0; i < count; i++)
                    gd_plot(x + i, y + r, color, op);
            memcpy(after, ram, size);
            memcpy(ram, before, size);
            gd_fill_select(&fill, color, op);
            fill.box(&fill, x, y, count, rows);
            if (memcmp(ram, after, size)) {
                fprintf(stderr, "mode %u op %u color %u x %u count %u y %u rows %u\n", modes[m].mode, op, color, x, count, y, rows);
                return 1;
            }
        }
    }
    puts("ok");
    return 0;
}
