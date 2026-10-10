/* Where an array's elements are on this target: DGROUP for a near array, the
   far heap's paragraphs for a far or huge one (QB rt/dynamic.asm, fhutil.asm).
   */
#include "qb.h"
#include "ad.h"
#include "far.h"
#include "fheap.h"

enum { SEGMENT = 0x10000UL };

int ad_allocated(const AD *ad)
{
    return ad->data_seg != 0;
}

void ad_use_near(AD *ad, void *data)
{
    ad->data_seg = dgroup_segment();
    ad->data_off = (u16)data;
    ad->adjusted += (u16)data;
}

void *ad_near_data(const AD *ad)
{
    return (void *)ad->data_off;
}

unsigned long ad_free_bytes(void)
{
    return fh_free_bytes();
}

void ad_far_alloc(AD *ad, unsigned long bytes)
{
    fh_alloc(ad, bytes);
}

/* A huge array too big for a segment starts at the offset that makes an element
   end where the segment does, so none straddles a segment's end. Past two
   segments that is refused, as QB refuses it. */
void ad_huge_alloc(AD *ad, unsigned long bytes)
{
    u16 shift;

    if (bytes >= SEGMENT && (shift = SEGMENT % ad->elem) != 0) {
        ad->data_off = shift;
        ad->adjusted += shift;
        bytes += shift;
        if (bytes >= 2 * SEGMENT)
            qb_error(BE_SUBSCRIP);
    }
    fh_alloc(ad, bytes);
}

void ad_far_free(AD *ad)
{
    fh_free(ad);
}

void ad_forget(AD *ad)
{
    ad->data_seg = 0;
}

void ad_near_moved(AD *ad, int delta)
{
    ad->data_off += delta;
    ad->adjusted += delta;
}

void ad_clear(AD *ad, unsigned long bytes)
{
    far_clear_bytes(ad->data_seg, ad->data_off, bytes);
}
