/* Where an array's elements are on this target: DGROUP for a near array, the
   far heap's paragraphs for a far or huge one (QB rt/dynamic.asm, fhutil.asm).
   */
#include "qb.h"
#include "ad.h"
#include "far.h"
#include "fheap.h"
#include "nheap.h"
#include "device.h"

#define SEGMENT 0x10000UL

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

/* A pointer is the segment in its high word: the paragraph arithmetic a huge array's two segments want. */
unsigned long ad_remaining(const AD *ad, qb_data_ptr from)
{
    unsigned long total = ad->elem, start, at = (unsigned long)from;
    unsigned dim;

    for (dim = 0; dim < ad->dims; dim++)
        total *= ad->dm[dim].count;
    start = ((unsigned long)ad->data_seg << 4) + ad->data_off;
    at = ((at >> 16) << 4) + (at & 0xFFFF);
    return at >= start && at - start <= total ? total - (at - start) : 0;
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

/* `count` bytes between far addresses given as a segment and a long offset. */
static void copy_far(u16 to_segment, unsigned long to, u16 from_segment, unsigned long from, unsigned long count)
{
    while (count) {
        unsigned long chunk = count < 0x1000 ? count : 0x1000;
        u8 __far *target = (u8 __far *)far_words(to_segment + (u16)(to >> 4)) + (to & 15);
        const u8 __far *source = (const u8 __far *)far_words(from_segment + (u16)(from >> 4)) + (from & 15);
        BlockOp op;

        op.dst = target;
        op.src = source;
        op.count = (unsigned)chunk;
        op.value = 0;
        dev_move(&op);
        to += chunk;
        from += chunk;
        count -= chunk;
    }
}

void ad_copy(AD *to, const AD *from, unsigned long bytes)
{
    copy_far(to->data_seg, to->data_off, from->data_seg, from->data_off, bytes);
}

void ad_adopt(AD *to, AD *from)
{
    if (from->features & (FADF_FAR | FADF_HUGE))
        fh_adopt(to, from);
    else {
        lh_entry((void *)from->data_off)->owner = to;
        to->data_seg = from->data_seg;
    }
    to->data_off = from->data_off;
    from->data_seg = 0;
}
