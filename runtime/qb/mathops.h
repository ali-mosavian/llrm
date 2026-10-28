/* The sine and cosine of an angle at once, which the target does in its FPU (dos/<mode>/fpu.asm) and a
   host in its math library. */
#ifndef QB_MATHOPS_H
#define QB_MATHOPS_H

#pragma aux qb_sincos "B$SINCOS"

extern void qb_sincos(const double *angle, double *sine, double *cosine);

#endif
