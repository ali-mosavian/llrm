/* A pixel blend of about 50 instructions called from a dozen places, one of them the inner loop of a
 * fill (961 times the caller's entry). The -Os size bound kept the call in the loop: QuickrBasic
 * `pixmem.c`, `q45d25` +62% instructions against clang. */
typedef unsigned char u8;
u8 *pixels;
unsigned width, mask;

static unsigned combined(unsigned old, unsigned color, unsigned operation)
{
    switch (operation & 3) {
    case 1: return old & color;
    case 2: return old | color;
    case 3: return old ^ color;
    default: return color;
    }
}

static void blend(unsigned x, unsigned y, unsigned color, unsigned operation)
{
    u8 *at = pixels + (unsigned long)y * width + x;

    *at = (u8)(combined(*at, color & mask, operation) & mask);
}

void plot(unsigned x, unsigned y, unsigned c, unsigned o) { blend(x, y, c, o); }
void dot(unsigned x, unsigned y, unsigned c) { blend(x, y, c, 0); }
void put(unsigned x, unsigned y, unsigned c) { blend(x, y, c, 3); }
void cross(unsigned x, unsigned y, unsigned c) { blend(x + 1, y, c, 0); blend(x, y + 1, c, 0); }
void mark1(unsigned x, unsigned y, unsigned c) { blend(x + 1, y, c, 1); }
void mark2(unsigned x, unsigned y, unsigned c) { blend(x + 2, y, c, 2); }
void mark3(unsigned x, unsigned y, unsigned c) { blend(x + 3, y, c, 3); }
void mark4(unsigned x, unsigned y, unsigned c) { blend(x + 4, y, c, 0); }
void mark5(unsigned x, unsigned y, unsigned c) { blend(x + 5, y, c, 1); }
void mark6(unsigned x, unsigned y, unsigned c) { blend(x + 6, y, c, 2); }
void mark7(unsigned x, unsigned y, unsigned c) { blend(x + 7, y, c, 3); }
void mark8(unsigned x, unsigned y, unsigned c) { blend(x + 8, y, c, 0); }
void mark9(unsigned x, unsigned y, unsigned c) { blend(x + 9, y, c, 1); }
void fill(unsigned x, unsigned y, unsigned n, unsigned rows, unsigned c, unsigned o)
{
    unsigned row, column;

    for (row = 0; row < rows; row++)
        for (column = 0; column < n; column++)
            blend(x + column, y + row, c, o);
}
