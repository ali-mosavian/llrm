/* Where an array's elements are on this target: the local heap, for every kind
   of array, since one pointer reaches all memory. */
#include "qb.h"
#include "ad.h"
#include "nheap.h"
#include "nhstutil.h"

int ad_allocated(const AD *ad)
{
    return ad->data != 0;
}

void ad_use_near(AD *ad, void *data)
{
    ad->data = data;
    ad->adjusted += (uword)data;
}

void *ad_near_data(const AD *ad)
{
    return ad->data;
}

unsigned long ad_remaining(const AD *ad, qb_data_ptr from)
{
    unsigned long total = ad->elem, start = (unsigned long)ad->data, at = (unsigned long)from;
    unsigned dim;

    for (dim = 0; dim < ad->dims; dim++)
        total *= ad->dm[dim].count;
    return at >= start && at - start <= total ? total - (at - start) : 0;
}

unsigned long ad_free_bytes(void)
{
    unsigned long largest = 0, room;
    const char *at = heap_low;

    while (at < heap_top) {
        const LhEntry *entry = (const LhEntry *)at;
        unsigned long room = entry->size - (sizeof(LhEntry) + sizeof(uword));

        if (entry->type == LH_FREE && room > largest)
            largest = room;
        at += entry->size;
    }
    /* the free end of string space, which the heap takes when it asks (lh_take_from_strings): the heap starts with none */
    room = str_free_bytes();
    if (room > sizeof(LhEntry) + sizeof(uword))
        room -= sizeof(LhEntry) + sizeof(uword);
    return room > largest ? room : largest;
}

void ad_far_alloc(AD *ad, unsigned long bytes)
{
    void *data = lh_alloc((uword)bytes, LH_ARRAY, ad, 0);

    if (!data)
        qb_error(BE_MEMORY);
    ad_use_near(ad, data);
}

void ad_huge_alloc(AD *ad, unsigned long bytes)
{
    ad_far_alloc(ad, bytes);
}

void ad_far_free(AD *ad)
{
    lh_free(ad->data);
    ad->data = 0;
}

void ad_forget(AD *ad)
{
    ad->data = 0;
}

void ad_near_moved(AD *ad, int delta)
{
    ad->data = (char *)ad->data + delta;
    ad->adjusted += delta;
}

void ad_clear(AD *ad, unsigned long bytes)
{
    char *at = ad->data;

    while (bytes--)
        *at++ = 0;
}

/* Into a fresh array (REDIM PRESERVE): the arrays are distinct, a block copy. */
void ad_copy(AD *to, const AD *from, unsigned long bytes)
{
    copy_bytes((char *)to->data, (const char *)from->data, (unsigned)bytes);
}

void ad_adopt(AD *to, AD *from)
{
    lh_entry(from->data)->owner = to;
    to->data = from->data;
    from->data = 0;
}
