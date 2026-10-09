/* Dynamic arrays (QB rt/dynamic.asm, rt/fhutil.asm). */
#ifndef QB_ARRAY_H
#define QB_ARRAY_H

#include "qb.h"
#include "ad.h"

/* The stack block of B$DDIM and B$RDIM as the frontend pushed it, last argument
   first: the descriptor, the rank (low byte) with the features (high), the
   element size, then each dimension's upper and lower bound, the one nearest
   the front being the first stored. */
typedef struct DimCall {
    AD *ad;
    u16 rank_and_features;
    u16 element;
    struct {
        int upper;
        int lower;
    } bounds[1];
} DimCall;

enum DimMode { DIM_ALLOCATE, DIM_REALLOCATE };

/* B$DDIM and B$RDIM: the asm entries (varargs.asm) call this. */
void array_dim(enum DimMode mode, const DimCall *call);

/* B$ERAS */
void B_ERAS(AD *ad);

#endif
