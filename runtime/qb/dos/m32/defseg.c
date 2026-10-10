/* DEF SEG (QB rt/rtinit.asm B$DSG0): the segment PEEK, POKE, BLOAD and the like address.  The compiled code reads and
   sets b$seg itself, so only DEF SEG with no segment comes here.

   An address is segment * 16 + offset, the offset as wide as a pointer.  VARSEG is always 0 and VARPTR the whole address,
   so DEF SEG = VARSEG(x) and PEEK(VARPTR(x)) reach x.  The default segment is 0: an offset is an address; a program
   names a paragraph only for the first megabyte's hardware (DEF SEG = &HB800 is video memory). */
#include "rtinit.h"

unsigned short b_seg;

void B_DSG0(void)
{
    b_seg = 0;
}

static Comp comp = { 0, C_RT, { B_DSG0 } };

#define XI_FN defseg_xinit
#include "xi.h"
void defseg_xinit(void)
{
    qb_comp_add(&comp);
}
#pragma aux b_seg "b$seg"
#pragma aux B_DSG0 "B$DSG0"
