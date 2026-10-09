/* The screen statements (QB rt/gwscr.asm B$COLR, B$LOCT, B$CSCN; rt/gwscreen
   B$SCLS, B$WIDT; prview B$VWPT). */
#include "console.h"

enum { COLOR_ARGUMENTS = 3, LOCATE_ARGUMENTS = 5 };

/* A statement's arguments from the block BASIC pushed: the count of words, then
   for each argument (the last at the lowest address) a flag and, only if the
   flag is not zero, the value.  `values` get -1 where an argument is left out.
   */
static void arguments(const int *block, int *values, unsigned slots)
{
    unsigned count = block[0], word = count, at;

    for (at = 0; at < slots; at++)
        values[at] = -1;
    for (at = 0; word > 0 && at < slots; at++) {
        int present = block[word--];

        if (present)
            values[at] = block[word--];
    }
}

/* B$COLR's block: COLOR foreground, background, border. */
void screen_color(const int *block)
{
    int value[COLOR_ARGUMENTS];

    arguments(block, value, COLOR_ARGUMENTS);
    cn_color(value[0], value[1]);
}

/* B$LOCT's block: LOCATE row, column, cursor, start, stop. */
void screen_locate(const int *block)
{
    int value[LOCATE_ARGUMENTS];

    arguments(block, value, LOCATE_ARGUMENTS);
    cn_locate(value[0], value[1]);
}

/* B$CSCN's block: SCREEN mode, colorswitch, active page, visible page.  Text
   mode 0 is the only one, and asking for the mode the screen is in leaves it as
   it is. */
void screen_mode(const int *block)
{
    int value[4];

    arguments(block, value, 4);
    if (value[0] > 0)
        qb_error(BE_ILLFUN);
}

/* B$SCLS: CLS, with -1 for no argument. */
void B_SCLS(int selector)
{
    (void)selector;
    cn_cls();
}

/* B$VWPT: VIEW PRINT top TO bottom; -1 and -1 for no bounds. */
void B_VWPT(int top, int bottom)
{
    cn_view(top, bottom);
}

/* B$WIDT: WIDTH columns, rows. */
void B_WIDT(int columns, int rows)
{
    cn_set_size(columns, rows);
}
#pragma aux screen_color "@screen_color@2"
#pragma aux screen_locate "@screen_locate@2"
#pragma aux screen_mode "@screen_mode@2"
#pragma aux B_SCLS "B$SCLS"
#pragma aux B_VWPT "B$VWPT"
#pragma aux B_WIDT "B$WIDT"
