/* Where an array's elements are on this target: the local heap, for every kind of array, since one pointer
   reaches all memory. */
#include "qb.h"
#include "ad.h"
#include "nheap.h"

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

unsigned long ad_free_bytes(void)
{
    unsigned long largest = 0;
    const char *at = heap_low;

    while (at < heap_top) {
        const LhEntry *entry = (const LhEntry *)at;
        unsigned long room = entry->size - (sizeof(LhEntry) + sizeof(uword));

        if (entry->type == LH_FREE && room > largest)
            largest = room;
        at += entry->size;
    }
    return largest;
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
