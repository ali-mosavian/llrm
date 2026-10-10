/* The array descriptor of this target, which the frontend emits and indexes (llrm-qb's AdLayout): QB's AD with
   whole pointers where QB's has an offset and a segment, no far heap, so no links or paragraphs, and every count, bound
   and size a word of the target (W bytes), not 16 bits.

   The part runtime/qb reads is the rank, features, element size, adjusted address and the bounds; where the
   elements are is the target's, behind the ad_* functions: here all in the local heap. */
#ifndef QB_AD_H
#define QB_AD_H

#include "platform.h"

typedef struct DM {
    uword count;
    int lbound;
} DM;

typedef struct AD {
    void *data;       /* 0: the elements, 0 when none */
    uword size;       /* W: bytes */
    u8 dims;          /* 2W */
    u8 features;      /* 2W + 1 */
    uword elem;       /* 3W: bytes of an element */
    uword adjusted;   /* 4W: where element (0,...,0) would be: the data less every lower bound's elements */
    DM dm[1];         /* 5W */
} AD;

/* Any array may hold what memory has. */
enum { AD_MAX_BYTES = 0x7FFFFFFFUL };

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

/* An array's elements, in the local heap at `data`. */
void ad_use_near(AD *ad, void *data);
void *ad_near_data(const AD *ad);

/* A far or huge array's elements: here the same, `bytes` of the local heap, zeroed. Freeing leaves the array without
   elements. */
void ad_far_alloc(AD *ad, unsigned long bytes);
void ad_huge_alloc(AD *ad, unsigned long bytes);
void ad_far_free(AD *ad);

/* The array without elements, once its data is freed. */
void ad_forget(AD *ad);

/* The local heap moved an array's elements `delta` bytes up. */
void ad_near_moved(AD *ad, int delta);

/* Zeros `bytes` of the elements, as ERASE of a static array does. */
void ad_clear(AD *ad, unsigned long bytes);

/* The bytes of the largest array the target can still take: FRE(-1). */
unsigned long ad_free_bytes(void);

#endif
