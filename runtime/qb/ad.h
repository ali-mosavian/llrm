/* The array descriptor (QB inc/array.inc AD and FHD), which the frontend emits
   and indexes (docs/frontends/qb/abi.md): the data's address, the next
   descriptor of the far heap, the size, then the rank, features, adjusted base
   and element width, then a DM per dimension. */
#ifndef QB_AD_H
#define QB_AD_H

#include "qb.h"

typedef struct DM {
    word count;
    int lbound;
} DM;

typedef struct AD {
    word data_off;   /* FHD_oData */
    word data_seg;   /* FHD_hData: DS for a near array, 0 when none */
    word next;       /* FHD_pNext: the next descriptor of the far heap */
    word size;       /* FHD_cPara: paragraphs, or bytes for a near array */
    byte dims;
    byte features;
    word adjusted;   /* AD_oAdjusted: the offset of element (0,...,0) */
    word elem;       /* AD_cbElement */
    DM dm[1];
} AD;

enum {
    FADF_NEAR = 0,
    FADF_FAR = 1,
    FADF_HUGE = 2,
    FADF_ALCMSK = 3,
    FADF_STATIC = 0x40,
    FADF_SD = 0x80
};

#endif
