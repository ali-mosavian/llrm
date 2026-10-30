/* Dynamic arrays (QB rt/dynamic.asm): DIM and REDIM fill the descriptor and
   allocate the elements, in the local heap for a near array and the far heap
   for a far or huge one; ERASE gives them back.  Where the elements are, and
   how they are reached, is the target's (ad.h). */
#include "array.h"
#include "nhstutil.h"
#include "rtinit.h"

/* The most an array may hold, unless it is huge: what the target's pointer can
 * reach. */
enum { ARRAY_MAX = AD_MAX_BYTES };

static int is_string_array(const AD *ad)
{
    return ad->features & FADF_SD;
}

/* B$ADArraySize: the bytes of the elements, or false where they exceed what
   four bytes can say. */
static int array_bytes(const AD *ad, unsigned long *bytes)
{
    unsigned long size = ad->elem;
    unsigned dim;

    for (dim = 0; dim < ad->dims; dim++) {
        if (ad->dm[dim].count && size > 0xFFFFFFFFUL / ad->dm[dim].count)
            return 0;
        size *= ad->dm[dim].count;
    }
    *bytes = size;
    return 1;
}

/* The bounds into the descriptor, and the offset that turns a subscript sum
   into an address: each dimension's count times what the dimensions before it
   came to, less its lower bound, times the element size, in a machine word. */
static void describe(AD *ad, const DimCall *call)
{
    unsigned dim;
    uword adjustment = 0;
    unsigned rank_and_features = (unsigned short)call->rank_and_features;

    ad->dims = rank_and_features & 0xFF;
    ad->features = rank_and_features >> 8;
    ad->elem = (unsigned short)call->element;
    for (dim = 0; dim < ad->dims; dim++) {
        int upper = (short)call->bounds[dim].upper, lower = (short)call->bounds[dim].lower;

        if (upper < lower)
            qb_error(BE_SUBSCRIP);
        ad->dm[dim].count = upper - lower + 1;
        ad->dm[dim].lbound = lower;
        adjustment = adjustment * ad->dm[dim].count - lower;
    }
    ad->adjusted = (uword)(adjustment * ad->elem);
}

/* A near array's elements, zeroed, in the local heap. */
static void allocate_near(AD *ad, unsigned bytes)
{
    void *data = lh_alloc(bytes, LH_ARRAY, ad, 0);

    if (!data)
        qb_error(BE_MEMORY);
    ad_use_near(ad, data);
}

/* The elements of a described array, zeroed, where its features say. */
static void place_elements(AD *ad, unsigned long bytes)
{
    if (ad->features & FADF_HUGE) {
        ad_huge_alloc(ad, bytes);
        return;
    }
    if (bytes > ARRAY_MAX)
        qb_error(BE_SUBSCRIP);
    if (ad->features & FADF_FAR)
        ad_far_alloc(ad, bytes);
    else
        allocate_near(ad, bytes);
}

/* REDIM PRESERVE of an array that has elements: only the outermost dimension may
   change its upper bound, and the elements of the old extent stay, the rest
   zeroed. The new elements are placed first, in a descriptor of the stack, so
   that old and new exist together; the old are then freed and the new descriptor
   adopted by the array's own, which the heap moves from then on. Anything else
   changed is a Subscript out of range, as in QB. A string array's strings belong
   to their descriptors: the kept ones follow to the new element, and the old are
   cleared so that erasing the old array frees only the strings cut off. */
static void preserve(AD *ad, const DimCall *call)
{
    union {
        AD ad;
        u8 room[sizeof(AD) + 59 * sizeof(DM)];
    } fresh;
    unsigned long old_bytes, new_bytes;
    unsigned dim, i;

    if (ad->features & FADF_STATIC)
        qb_error(BE_REDIM);
    for (i = 0; i < sizeof fresh; i++)
        fresh.room[i] = 0;
    describe(&fresh.ad, call);
    if (fresh.ad.dims != ad->dims || fresh.ad.elem != ad->elem)
        qb_error(BE_SUBSCRIP);
    for (dim = 0; dim < ad->dims; dim++) {
        int outer = dim + 1 == ad->dims;

        if (fresh.ad.dm[dim].lbound != ad->dm[dim].lbound || (!outer && fresh.ad.dm[dim].count != ad->dm[dim].count))
            qb_error(BE_SUBSCRIP);
    }
    array_bytes(ad, &old_bytes);
    if (!array_bytes(&fresh.ad, &new_bytes))
        qb_error(BE_SUBSCRIP);
    fresh.ad.size = new_bytes;
    place_elements(&fresh.ad, new_bytes);
    ad_copy(&fresh.ad, ad, old_bytes < new_bytes ? old_bytes : new_bytes);
    if (is_string_array(ad)) {
        SD *from = ad_near_data(ad), *to = ad_near_data(&fresh.ad);
        unsigned kept = (unsigned)((old_bytes < new_bytes ? old_bytes : new_bytes) / sizeof(SD));

#pragma aux B_ERAS "B$ERAS"
#pragma aux array_dim "@array_dim@4"

        for (i = 0; i < kept; i++) {
            str_owner_moved(&to[i], (int)((char *)&to[i] - (char *)&from[i]));
            from[i].len = 0;
            from[i].ptr = 0;
        }
    }
    B_ERAS(ad);
    ad_adopt(ad, &fresh.ad);
    ad->adjusted = fresh.ad.adjusted;
    ad->size = fresh.ad.size;
    for (dim = 0; dim < ad->dims; dim++)
        ad->dm[dim] = fresh.ad.dm[dim];
}

/* DIM, and REDIM after it has erased the array: describes the array and
   allocates its elements.  Dimensioning an array that has elements is a
   Duplicate definition, and elements past 64 KB are a Subscript out of range
   unless the array is huge. */
void array_dim(enum DimMode mode, const DimCall *call)
{
    AD *ad = call->ad;
    unsigned long bytes;

    if (mode == DIM_PRESERVE && ad_allocated(ad)) {
        preserve(ad, call);
        return;
    }
    if (mode != DIM_ALLOCATE)
        B_ERAS(ad);
    if (ad_allocated(ad))
        qb_error(BE_REDIM);
    describe(ad, call);
    if (!array_bytes(ad, &bytes))
        qb_error(BE_SUBSCRIP);
    ad->size = bytes;
    place_elements(ad, bytes);
}

/* A string array's strings are freed with it. */
static void release_strings(const AD *ad)
{
    SD *sd = ad_near_data(ad);
    unsigned long bytes;
    unsigned strings;

    array_bytes(ad, &bytes);
    for (strings = bytes / sizeof(SD); strings; strings--, sd++)
        str_release(sd);
}

/* B$ERAS: a static array is cleared; a dynamic one is freed and its descriptor
   left without elements. */
void B_ERAS(AD *ad)
{
    unsigned long bytes;

    if (!ad_allocated(ad))
        return;
    array_bytes(ad, &bytes);
    if (ad->features & FADF_STATIC) {
        if (is_string_array(ad))
            release_strings(ad);
        ad_clear(ad, bytes);
        return;
    }
    if (is_string_array(ad)) {
        release_strings(ad);
        lh_free(ad_near_data(ad));
        ad_forget(ad);
    } else {
        ad_far_free(ad);
    }
    ad->size = 0;
}

/* The local heap moved the elements of a dynamic string array `delta` bytes up:
   their strings and the descriptor follow (B$LHADJ). */
static void moved(void *data, int delta)
{
    LhEntry *entry = lh_entry(data);
    AD *ad = entry->owner;

    if (is_string_array(ad)) {
        SD *sd = data;
        unsigned strings;
        unsigned capacity = entry->size - sizeof(LhEntry) - sizeof(uword);

        for (strings = capacity / sizeof(SD); strings; strings--, sd++)
            str_owner_moved(sd, delta);
    }
    ad_near_moved(ad, delta);
}

#define XI_FN array_xinit
#include "xi.h"

void array_xinit(void)
{
    lh_on_move(LH_ARRAY, moved);
}
