/* QB's component dispatch (rtinit.asm b$ini_disp and its siblings,
   inc/compvect.inc).  A module registers one Comp from its initializer; the
   runtime runs the slots it set. */
#ifndef QB_RTINIT_H
#define QB_RTINIT_H

#include "qb.h"

typedef void (__far *Vec)(void);

/* The slots of compvect.inc: ini, run, clrt and err run in id order; end and
   term in reverse. */
enum { V_INI, V_RUN, V_CLRT, V_ERR, V_END, V_TERM, V_COUNT };

/* Component ids, in QB's initialization order. */
enum { C_NH, C_FH, C_RT, C_DV, C_CN, C_OS, C_SN, C_DB, C_MT, C_GR, C_EV, C_DK };

typedef struct Comp {
    struct Comp *next;
    byte id;
    Vec v[V_COUNT];
} Comp;

void qb_comp_add(Comp *c);
void qb_dispatch(byte slot);
byte qb_rt_inited(void);

#endif
