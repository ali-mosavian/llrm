/* Dynamic arrays (QB rt/dynamic.asm): DIM and REDIM fill the descriptor and
   allocate the elements, in the local heap for a near array and the far heap
   for a far or huge one; ERASE gives them back. */
#include "array.h"
#include "far.h"
#include "fheap.h"
#include "nhstutil.h"
#include "rtinit.h"

enum { SEGMENT = 0x10000UL };

static int is_string_array(const AD *ad)
{
    return ad->features & FADF_SD;
}

/* B$ADArraySize: the bytes of the elements, or false where they exceed what
   four bytes can say. */
static int array_bytes(const AD *ad, unsigned long *bytes)
{
    unsigned long size = ad->elem;
    word dim;

    for (dim = 0; dim < ad->dims; dim++) {
        if (ad->dm[dim].count && size > 0xFFFFFFFFUL / ad->dm[dim].count)
            return 0;
        size *= ad->dm[dim].count;
    }
    *bytes = size;
    return 1;
}

/* The bounds into the descriptor, and the offset that turns a subscript sum
   into an address. */
static void describe(AD *ad, const DimCall *call)
{
    word dim, adjustment = 0;

    ad->dims = call->rank_and_features & 0xFF;
    ad->features = call->rank_and_features >> 8;
    ad->elem = call->element;
    for (dim = 0; dim < ad->dims; dim++) {
        int upper = call->bounds[dim].upper, lower = call->bounds[dim].lower;

        if (upper < lower)
            qb_error(BE_SUBSCRIP);
        ad->dm[dim].count = upper - lower + 1;
        ad->dm[dim].lbound = lower;
        adjustment = adjustment * ad->dm[dim].count - lower;
    }
    ad->adjusted = adjustment * ad->elem;
}

static void allocate_near(AD *ad, word bytes)
{
    word data = (word)lh_alloc(bytes, LH_ARRAY, ad, 0);

    if (!data)
        qb_error(BE_MEMORY);
    ad->data_seg = dgroup_segment();
    ad->data_off = data;
    ad->adjusted += data;
}

/* A huge array may be bigger than a segment: it starts at the offset that makes
   an element end where the segment does, so no element straddles a segment's
   end. */
static unsigned long align_huge(AD *ad, unsigned long bytes)
{
    word shift;

    if (bytes < SEGMENT)
        return bytes;
    shift = SEGMENT % ad->elem;
    if (!shift)
        return bytes;
    ad->data_off = shift;
    ad->adjusted += shift;
    bytes += shift;
    if (bytes >= 2 * SEGMENT)
        qb_error(BE_SUBSCRIP);
    return bytes;
}

void array_dim(enum DimMode mode, const DimCall *call)
{
    AD *ad = call->ad;
    unsigned long bytes;

    if (mode == DIM_REALLOCATE)
        B_ERAS(ad);
    if (ad->data_seg)
        qb_error(BE_REDIM);
    describe(ad, call);
    if (!array_bytes(ad, &bytes))
        qb_error(BE_SUBSCRIP);
    ad->size = bytes;
    ad->data_off = 0;
    if (ad->features & FADF_HUGE) {
        fh_alloc(ad, align_huge(ad, bytes));
    } else {
        if (bytes > SEGMENT)
            qb_error(BE_SUBSCRIP);
        if (ad->features & FADF_FAR)
            fh_alloc(ad, bytes);
        else
            allocate_near(ad, bytes);
    }
}

/* A string array's strings are freed with it. */
static void release_strings(const AD *ad)
{
    SD *sd = (SD *)ad->data_off;
    unsigned long bytes;
    word strings;

    array_bytes(ad, &bytes);
    for (strings = bytes / sizeof(SD); strings; strings--, sd++)
        str_release(sd);
}

/* B$ERAS: a static array is cleared; a dynamic one is freed and its descriptor
   left unallocated. */
void B_ERAS(AD *ad)
{
    unsigned long bytes;

    if (!ad->data_seg)
        return;
    array_bytes(ad, &bytes);
    if (ad->features & FADF_STATIC) {
        if (is_string_array(ad))
            release_strings(ad);
        far_clear_bytes(ad->data_seg, ad->data_off, bytes);
        return;
    }
    if (is_string_array(ad)) {
        release_strings(ad);
        lh_free((void *)ad->data_off);
        ad->data_seg = 0;
    } else {
        fh_free(ad);
    }
    ad->size = 0;
}

/* The local heap moved the elements of a dynamic string array `delta` bytes up:
   their strings and the descriptor follow (B$LHADJ). */
static void moved(void *data, int delta)
{
    LhEntry *entry = lh_entry(data);
    AD *ad = entry->owner;
    SD *sd = data;
    word strings;

    word capacity = entry->size - sizeof(LhEntry) - sizeof(word);

    for (strings = capacity / sizeof(SD); strings; strings--, sd++)
        str_owner_moved(sd, delta);
    ad->data_off += delta;
    ad->adjusted += delta;
}

#define XI_FN array_xinit
#include "xi.h"
void array_xinit(void)
{
    lh_on_move(LH_ARRAY, moved);
}
#pragma aux B_ERAS "B$ERAS"
#pragma aux array_dim "@array_dim@4"
