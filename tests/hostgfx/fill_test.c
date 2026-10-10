/* The box fills, dots and CGA searches of runtime/qb/dos/gfxdev.c against the pixel plot and read, on the host: for the 256-colour and the two CGA modes, every
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

/* fill.asm in C: the run of `op mem, imm`, and the edge bytes. */
static unsigned fill_operation, fill_byte;
void dev_fill_select(unsigned operation, unsigned byte) { fill_operation = operation; fill_byte = byte; }
static unsigned apply(unsigned old) { return fill_operation == 0 ? fill_byte : fill_operation == 1 ? old & fill_byte : fill_operation == 2 ? old | fill_byte : old ^ fill_byte; }
void dev_fill_box(FillBox *box)
{
    unsigned char *row = box->dst;
    unsigned phase = box->phase;

    for (unsigned r = 0; r < box->rows; r++) {
        unsigned char *at = row;

        if (box->edges) {
            *at = (*at & (box->left & 0xFF)) ^ (box->left >> 8 & 0xFF);
            at++;
        }
        for (unsigned i = 0; i < box->middle; i++, at++)
            *at = (unsigned char)apply(*at);
        if (box->edges)
            *at = (*at & (box->right & 0xFF)) ^ (box->right >> 8 & 0xFF);
        row += (int)box->steps[phase / sizeof(unsigned)];   /* a step back wraps as the target's offsets do */
        phase ^= sizeof(unsigned);
    }
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
            /* the dot of the same fill, on random pixels, against the plot */
            for (unsigned i = 0; i < 50; i++) {
                unsigned dx = next() % modes[m].width, dy = next() % 200;

                memcpy(ram, before, size);
                gd_plot(dx, dy, color, op);
                memcpy(after, ram, size);
                memcpy(ram, before, size);
                gd_dots_begin(&fill);
                fill.dot(&fill, dx, dy);
                gd_dots_end(&fill);
                if (memcmp(ram, after, size)) {
                    fprintf(stderr, "dot: mode %u op %u color %u x %u y %u\n", modes[m].mode, op, color, dx, dy);
                    return 1;
                }
            }
            /* a search along a row of few colours, against a pixel at a time */
            for (unsigned i = 0; i < 20; i++) {
                unsigned sy = next() % 200, c1 = next() % modes[m].colors % 3, c2 = next() % modes[m].colors % 3;
                int sx = next() % modes[m].width, sl = next() % modes[m].width, match = next() & 1, want = -1;

                for (unsigned px = 0; px < modes[m].width; px++)
                    gd_plot(px, sy, next() % 3 ? next() % modes[m].colors % 3 : 0, 0);
                if (next() & 1)
                    for (unsigned px = 0; px < modes[m].width; px++)
                        gd_plot(px, sy, c1, 0);
                for (int px = sx, step = sl >= sx ? 1 : -1;; px += step) {
                    unsigned pc = gd_read(px, sy);

                    if ((pc == c1 || pc == c2) == match) {
                        want = px;
                        break;
                    }
                    if (px == sl)
                        break;
                }
                if (gd_search(sx, sl, sy, c1, c2, match) != want) {
                    fprintf(stderr, "search: mode %u x %d last %d y %u c1 %u c2 %u match %d want %d got %d\n", modes[m].mode, sx, sl, sy, c1, c2, match, want, gd_search(sx, sl, sy, c1, c2, match));
                    return 1;
                }
            }
        }
    }
    puts("ok");
    return 0;
}
