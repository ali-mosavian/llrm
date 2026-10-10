/* The box fills, dots and CGA searches of runtime/qb/dos/gfxdev.c against the pixel plot and read, and lines against the dot, on the host: for the 256-colour and the two CGA modes, every
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

/* The line loops of fill.asm in C, a pixel at a time (the loops accumulate a byte's pixels, and the planar one is the controller's:
   not modelled): the style rotated through, the colour set, a step along x moving the byte or, in CGA, the mask, and a step in y moving
   the row or the CGA bank. */
static void walk(FillLine *line, int packed)
{
    unsigned char *at = line->dst;
    unsigned mask = line->pmask, style = line->style & 0xFFFF;
    int decision = line->decision;

    for (unsigned k = 0; k < line->count; k++) {
        int x_step, y_step;

        if (style & 0x8000) {
            if (packed)
                *at = (*at & ~mask) | (line->color & mask);
            else
                *at = (unsigned char)apply(*at);
        }
        style = (style << 1 | style >> 15) & 0xFFFF;
        if (line->x_major == 2) {
            x_step = 0;
            y_step = 1;
        } else {
            x_step = line->x_major == 1 || decision >= 0;
            y_step = line->x_major == 0 || decision >= 0;
            decision += decision < 0 ? line->minor4 : line->step4;
        }
        if (x_step) {
            if (!packed) {
                at++;
            } else {
                unsigned wraps = mask & ((1u << line->bpp) - 1);

                mask = (mask >> line->bpp | mask << (8 - line->bpp)) & 0xFF;
                if (wraps)
                    at++;
            }
        }
        if (y_step) {
            if (!packed) {
                at += line->ystep;
            } else {
                unsigned off = (unsigned)(at - cga_ram) ^ 0x2000;

                if (line->ystep > 0 ? !(off & 0x2000) : (off & 0x2000))
                    off += line->ystep;
                at = cga_ram + off;
            }
        }
    }
}
void dev_line_linear(FillLine *line) { walk(line, 0); }
void dev_line_packed(FillLine *line) { walk(line, 1); }
void dev_line_planar(FillLine *line) { (void)line; }

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
            if (modes[m].mode != 0x10 && modes[m].mode != 0x0D && op == 0) {
                /* a line with a style, against Bresenham's loop through the dot */
                unsigned lx = next() % 100, ly = 50 + next() % 100, ldx = next() % 100, ldy = next() % 50;
                unsigned style = next() & 1 ? 0xFFFF : (next() & 0xFFFF), turn = next() & 15;
                int up = next() & 1, step_y = up ? -1 : 1, major, minor, decision, px = (int)lx, py = (int)ly;

                if (next() % 4 == 0)
                    ldx = 0;
                major = (int)(ldx > ldy ? ldx : ldy);
                minor = (int)(ldx > ldy ? ldy : ldx);
                decision = 4 * minor - major;
                memcpy(ram, before, size);
                for (int k = 0; k <= major; k++) {
                    if (style & (0x8000u >> ((turn + k) & 15)))
                        fill.dot(&fill, (unsigned)px, (unsigned)py);
                    if (decision < 0) decision += 4 * minor; else { decision += 4 * (minor - major); if (ldx > ldy) py += step_y; else px++; }
                    if (ldx > ldy) px++; else py += step_y;
                }
                memcpy(after, ram, size);
                memcpy(ram, before, size);
                fill.style = turn ? (style << turn | style >> (16 - turn)) & 0xFFFF : style;
                fill.line(&fill, lx, ly, ldx, ldy, step_y);
                if (memcmp(ram, after, size)) {
                    fprintf(stderr, "line: mode %u color %u from %u,%u dx %u dy %u up %d style %x turn %u\n", modes[m].mode, color, lx, ly, ldx, ldy, up, style, turn);
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
