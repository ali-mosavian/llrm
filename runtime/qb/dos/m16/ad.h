/* The array descriptor of this target (QB inc/array.inc AD and FHD), which the
   frontend emits and indexes (docs/frontends/qb/abi.md), and what the portable
   array code asks of the target's memory model.

   The part runtime/qb reads is the rank, features, element size, adjusted
   offset and the bounds; where the elements are (a segment and offset, and the
   far heap's links) is the target's, behind the ad_* functions. */
#ifndef QB_AD_H
#define QB_AD_H

#include "platform.h"

typedef struct DM {
    u16 count;
    int lbound;
} DM;

typedef struct AD {
    u16 data_off;    /* FHD_oData */
    u16 data_seg;    /* FHD_hData: DS for a near array, 0 when none */
    u16 next;        /* FHD_pNext: the next descriptor of the far heap */
    u16 size;        /* FHD_cPara: paragraphs, or bytes for a near array */
    u8 dims;
    u8 features;
    u16 adjusted;    /* AD_oAdjusted: the offset of element (0,...,0) */
    u16 elem;        /* AD_cbElement */
    DM dm[1];
} AD;

/* The most bytes an array may have unless it is huge: a segment's. */
enum { AD_MAX_BYTES = 0x10000UL };

enum {
    FADF_NEAR = 0,
    FADF_FAR = 1,
    FADF_HUGE = 2,
    FADF_ALCMSK = 3,
    FADF_STATIC = 0x40,
    FADF_SD = 0x80
};

/* Whether the array has elements. */
int ad_allocated(const AD *ad);

/* A near array's elements, in the local heap at `data`. */
void ad_use_near(AD *ad, void *data);
void *ad_near_data(const AD *ad);

/* A far or huge array's elements: `bytes` of the far heap, zeroed. A huge array
   larger than a segment starts at an offset that ends an element where a
   segment ends. Freeing leaves the array without elements. */
void ad_far_alloc(AD *ad, unsigned long bytes);
void ad_huge_alloc(AD *ad, unsigned long bytes);
void ad_far_free(AD *ad);

/* The array without elements, once its near data is freed. */
void ad_forget(AD *ad);

/* The local heap moved a near array's elements `delta` bytes up. */
void ad_near_moved(AD *ad, int delta);

/* Zeros `bytes` of the elements, as ERASE of a static array does. */
void ad_clear(AD *ad, unsigned long bytes);

/* The bytes of the largest array the target can still take: FRE(-1). */
unsigned long ad_free_bytes(void);

#endif
