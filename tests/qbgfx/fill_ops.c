/* The box fills of runtime/qb/dos/gfxdev.c on the screen, every mode and every operation: a random box over random pixels leaves
   what the operation says of each pixel, and the pixels around it as they were; so does a dot of the same fill, and a line is what Bresenham's loop through the dot draws, and GET and PUT of a box are what the pixel at a time make.  Prints 0, or the first case that differed. */
#include "gfxdev.h"

extern void report(long value);

enum { REGION_W = 48, REGION_H = 24, ROUNDS = 120 };

static unsigned long rng = 12345;
static unsigned next(void)
{
    rng = rng * 1103515245UL + 12345UL;
    return (unsigned)(rng >> 16) & 0x7FFF;
}

static unsigned before[REGION_W * REGION_H], expected[REGION_W * REGION_H], saved[REGION_W * REGION_H];

static void grab(unsigned *to)
{
    unsigned x, y;

    for (y = 0; y < REGION_H; y++)
        for (x = 0; x < REGION_W; x++)
            to[y * REGION_W + x] = gd_read(x, y);
}

static void put_back(const unsigned *from)
{
    unsigned x, y;

    for (y = 0; y < REGION_H; y++)
        for (x = 0; x < REGION_W; x++)
            gd_plot(x, y, from[y * REGION_W + x], 0);
}

static unsigned char sprite[1024], expected_sprite[1024];

/* QB's array format by pixel: the bytes of the box as GET's old per-pixel code made them. */
static unsigned sprite_bytes(unsigned mode, unsigned w, unsigned h)
{
    unsigned planes = mode == 0x10 || mode == 0x0D ? 4 : 1, bits = mode == 0x13 ? 8 : mode == 4 ? 2 : 1;

    return h * planes * ((w * bits + 7) / 8);
}

static void pack_ref(unsigned mode, unsigned bx, unsigned by, unsigned w, unsigned h, unsigned char *out)
{
    unsigned planes = mode == 0x10 || mode == 0x0D ? 4 : 1, bits = mode == 0x13 ? 8 : mode == 4 ? 2 : 1;
    unsigned per_plane = (w * bits + 7) / 8, x, y, i;

    for (y = 0; y < h; y++, out += planes * per_plane) {
        for (i = 0; i < planes * per_plane; i++)
            out[i] = 0;
        for (x = 0; x < w; x++) {
            unsigned color = gd_read(bx + x, by + y), plane, at = x * bits;

            if (planes > 1) {
                for (plane = 0; plane < planes; plane++)
                    if (color >> plane & 1)
                        out[plane * per_plane + (x >> 3)] |= 0x80 >> (x & 7);
            } else {
                out[at >> 3] |= (unsigned char)(color << (8 - bits - (at & 7)));
            }
        }
    }
}

static void put_ref(unsigned mode, unsigned bx, unsigned by, unsigned w, unsigned h, const unsigned char *in, unsigned operation, unsigned invert, unsigned colors)
{
    unsigned planes = mode == 0x10 || mode == 0x0D ? 4 : 1, bits = mode == 0x13 ? 8 : mode == 4 ? 2 : 1;
    unsigned per_plane = (w * bits + 7) / 8, x, y;

    for (y = 0; y < h; y++, in += planes * per_plane)
        for (x = 0; x < w; x++) {
            unsigned color = 0, plane, at = x * bits;

            if (planes > 1) {
                for (plane = 0; plane < planes; plane++)
                    if (in[plane * per_plane + (x >> 3)] & (0x80 >> (x & 7)))
                        color |= 1u << plane;
            } else {
                color = in[at >> 3] >> (8 - bits - (at & 7)) & ((1u << bits) - 1);
            }
            if (invert)
                color = ~color & (colors - 1);
            gd_plot(bx + x, by + y, color, operation);
        }
}

int main(void)
{
    static const unsigned modes[] = { 0x13, 4, 6, 0x10, 0x0D };
    static const unsigned colors[] = { 256, 4, 2, 16, 16 };
    unsigned m, round, x, y;

    for (m = 0; m < sizeof(modes) / sizeof(modes[0]); m++) {
        gd_set_mode(modes[m]);
        for (round = 0; round < ROUNDS; round++) {
            unsigned operation = round & 3, color = next() % colors[m];
            unsigned bx = next() % REGION_W, by = next() % REGION_H;
            unsigned count = 1 + next() % (REGION_W - bx), rows = 1 + next() % (REGION_H - by);
            GdFill fill;

            for (y = 0; y < REGION_H; y++)
                for (x = 0; x < REGION_W; x++) {
                    gd_plot(x, y, next() % colors[m], 0);
                    before[y * REGION_W + x] = gd_read(x, y);
                }
            gd_fill_select(&fill, color, operation);
            fill.box(&fill, bx, by, count, rows);
            for (y = 0; y < REGION_H; y++)
                for (x = 0; x < REGION_W; x++) {
                    unsigned old = before[y * REGION_W + x], want = old;

                    if (x >= bx && x < bx + count && y >= by && y < by + rows)
                        want = (operation == 0 ? color : operation == 1 ? old & color : operation == 2 ? old | color : old ^ color) & (colors[m] - 1);
                    if (gd_read(x, y) != want) {
                        report((long)modes[m] * 1000000L + (long)operation * 100000L + (long)(bx + by * 100) + 1);
                        return 0;
                    }
                }
            /* GET and PUT of a box: whole rows against a pixel at a time */
            {
                unsigned sx = next() % REGION_W, sy = next() % REGION_H, sw = 1 + next() % (REGION_W - sx), sh = 1 + next() % (REGION_H - sy);
                unsigned bytes = sprite_bytes(modes[m], sw, sh), i, invert = operation == 0 && (next() & 1);

                if (sw > 40)
                    sw = 40;
                bytes = sprite_bytes(modes[m], sw, sh);
                pack_ref(modes[m], sx, sy, sw, sh, expected_sprite);
                gd_get(sx, sy, sw, sh, sprite);
                for (i = 0; i < bytes; i++)
                    if (sprite[i] != expected_sprite[i]) {
                        report((long)modes[m] * 1000000L + (long)operation * 100000L + (long)(sx + sy * 100) + 80000L);
                        return 0;
                    }
                for (i = 0; i < bytes; i++)
                    sprite[i] = (unsigned char)next();
                grab(saved);
                put_ref(modes[m], sx, sy, sw, sh, sprite, operation, invert, colors[m]);
                grab(expected);
                put_back(saved);
                gd_put(sx, sy, sw, sh, sprite, operation, invert);
                for (y = 0; y < REGION_H; y++)
                    for (x = 0; x < REGION_W; x++)
                        if (gd_read(x, y) != expected[y * REGION_W + x]) {
                            report((long)modes[m] * 1000000L + (long)operation * 100000L + (long)(sx + sy * 100) + 90000L);
                            return 0;
                        }
            }
            /* and a line with a style (set only): the loop of fill.asm against Bresenham's through the dot */
            if (operation == 0) {
                unsigned lx = next() % REGION_W, ly = next() % REGION_H, ldx = next() % (REGION_W - lx);
                int up = next() & 1, step_y = up ? -1 : 1;
                unsigned room = up ? ly : REGION_H - 1 - ly, ldy = room ? next() % (room + 1) : 0;
                unsigned style = next() & 1 ? 0xFFFF : (next() & 0xFFFF), turn = next() & 15;
                int major, minor, decision, k;
                int px = (int)lx, py = (int)ly;

                if (next() % 4 == 0)
                    ldx = 0;
                major = (int)(ldx > ldy ? ldx : ldy);
                minor = (int)(ldx > ldy ? ldy : ldx);
                decision = 4 * minor - major;
                grab(saved);
                gd_dots_begin(&fill);
                for (k = 0; k <= major; k++) {
                    if (style & (0x8000u >> ((turn + k) & 15)))
                        fill.dot(&fill, (unsigned)px, (unsigned)py);
                    if (decision < 0) {
                        decision += 4 * minor;
                    } else {
                        decision += 4 * (minor - major);
                        if (ldx > ldy)
                            py += step_y;
                        else
                            px++;
                    }
                    if (ldx > ldy)
                        px++;
                    else
                        py += step_y;
                }
                gd_dots_end(&fill);
                grab(expected);
                put_back(saved);
                fill.style = turn ? (style << turn | style >> (16 - turn)) & 0xFFFF : style;
                fill.line(&fill, lx, ly, ldx, ldy, step_y);
                for (y = 0; y < REGION_H; y++)
                    for (x = 0; x < REGION_W; x++)
                        if (gd_read(x, y) != expected[y * REGION_W + x]) {
                            report((long)modes[m] * 1000000L + (long)(lx + ly * 100) + 70000L + (ldx == 0 ? 5000L : 0));
                            return 0;
                        }
            }
            /* and a few dots of the same fill: the pixel as the operation says, its neighbours as they were */
            for (x = 0; x < 6; x++) {
                unsigned dx = 1 + next() % (REGION_W - 2), dy = next() % REGION_H;
                unsigned left = gd_read(dx - 1, dy), old = gd_read(dx, dy), right = gd_read(dx + 1, dy);
                unsigned want = (operation == 0 ? color : operation == 1 ? old & color : operation == 2 ? old | color : old ^ color) & (colors[m] - 1);

                gd_dots_begin(&fill);
                fill.dot(&fill, dx, dy);
                gd_dots_end(&fill);
                if (gd_read(dx, dy) != want || gd_read(dx - 1, dy) != left || gd_read(dx + 1, dy) != right) {
                    report((long)modes[m] * 1000000L + (long)operation * 100000L + (long)(dx + dy * 100) + 50000L);
                    return 0;
                }
            }
        }
    }
    report(0);
    return 0;
}
