/* DEF SEG (QB rt/rtinit.asm B$DSG0): the segment PEEK, POKE, BLOAD and the like address.  The compiled code reads and
   sets b$seg itself, so only DEF SEG with no segment comes here.

   An address is segment * 16 + offset, as in real mode; VARSEG gives an address's paragraph and VARPTR the rest, so
   DEF SEG = VARSEG(x) and PEEK(VARPTR(x)) reach x.  The default segment is 0: the offsets are then addresses of the
   first 64 KB. */
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
