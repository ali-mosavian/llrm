/* DEF SEG (QB rt/rtinit.asm B$DSG0): the segment PEEK, POKE, BLOAD and the like
   address.  The compiled code reads and sets b$seg itself, so only DEF SEG with
   no segment comes here. */
#include "far.h"
#include "rtinit.h"

word b_seg;

/* DEF SEG with no segment: the data segment. */
void B_DSG0(void)
{
    b_seg = dgroup_segment();
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
