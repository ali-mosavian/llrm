/* The local heap and the boundary it shares with string space (QB rt/nhlhcore.asm, nhlhutil.asm,
   nhinit.asm).  See nheap.h for the layout. */
#include "nhstutil.h"
#include "ad.h"
#include "rtinit.h"

word nh_first, nh_last;
word heap_first, heap_free, heap_end;

/* An FDB entry that moved `delta` bytes up (file.c sets it); the heap knows nothing else of it. */
void (*lh_file_moved)(word entry, word delta);

/* The entry pointer for the data pointer a caller was given. */
word lh_entry(word data)
{
    word back = data - 2;

    return back + W(back) - 1;
}

static word entry_data(word p)
{
    return p - LHLEN(p) + 3;
}

static word entry_size(word bytes, byte type)
{
    word size = (bytes + LH_STD_HDR + 2 + 1) & ~1u;

    return type == LH_FILE ? size + (LH_FDB_HDR - LH_STD_HDR) : size;
}

/* An entry of `size` bytes at p, with its trailing length in the entry below. */
static void set_entry(word p, word size, byte type)
{
    LHTYPE(p) = type;
    LHLEN(p) = size;
    W(p - size + 1) = size;
}

/* B$LH_ALC_FREE: allocate from the entry at heap_free; 0 when it does not fit. */
static word alloc_free(word size, byte type, word owner)
{
    word p = heap_free, len, rest, at;

    if (LHTYPE(p) != LH_FREE)
        return 0;
    len = LHLEN(p);
    if (len < size)
        return 0;
    rest = len - size;
    if (rest < LH_STD_HDR) {
        size = len;
        rest = 0;
    }
    if (rest)
        set_entry(p - size, rest, LH_FREE);
    heap_free = p - size;
    for (at = p - size + 1; at < p - 3; at += 2)
        W(at) = 0;
    set_entry(p, size, type);
    LHFNUM(p) = type == LH_FILE ? (byte)owner : 0;
    if (type != LH_FILE)
        LHBAKP(p) = owner;
    return entry_data(p);
}

/* B$LH_SCAN: first fit over the whole heap, joining free neighbours as it goes. */
static word scan(word size, byte type, word owner)
{
    word p = heap_first, len, next;

    while (LHTYPE(p) != LH_END) {
        if (LHTYPE(p) != LH_FREE) {
            p -= LHLEN(p);
            continue;
        }
        len = LHLEN(p);
        for (next = p - len; LHTYPE(next) == LH_FREE; next = p - len)
            len += LHLEN(next);
        set_entry(p, len, LH_FREE);
        if (len >= size) {
            heap_free = p;
            return alloc_free(size, type, owner);
        }
        p = next;
    }
    return 0;
}

/* B$LHSetFree: heap_free names the free entry just above END, or END itself. */
void lh_set_free(void)
{
    word p = heap_free, last;

    if (LHTYPE(p) == LH_FREE && p - LHLEN(p) == heap_end)
        return;
    p = heap_end;
    if (p != heap_first) {
        last = p + W(p + 1);
        if (LHTYPE(last) == LH_FREE)
            p = last;
    }
    heap_free = p;
}

/* B$LH_FROM_SS: take the free string space at the end of string space into the heap, as a free
   entry (or as more of the one already there). */
word lh_from_ss(void)
{
    word at, len, p, last;

    str_set_free();
    at = str_free;
    len = str_end - at;
    if (len == 0)
        return 0;
    if (heap_end != heap_first && LHTYPE(last = heap_end + W(heap_end + 1)) == LH_FREE) {
        p = last;
        len += LHLEN(p);
    } else {
        if (len < LH_STD_HDR + 2)
            return 0;
        p = at + LH_STD_HDR + 1 + len;
    }
    W(at) = 0xFFFF;
    str_end = str_free = at;
    heap_end = at + LH_STD_HDR + 1;
    LHTYPE(heap_end) = LH_END;
    set_entry(p, len, LH_FREE);
    heap_free = p;
    return 1;
}

/* B$LH_CPCT: slide every allocated entry up over the free ones, adjusting what points at it, so the
   free space is one entry beside string space. */
void lh_compact(void)
{
    word src = heap_first, dst, len, delta, from, to, n;

    while (LHTYPE(src) != LH_END && LHTYPE(src) != LH_FREE)
        src -= LHLEN(src);
    if (LHTYPE(src) == LH_END)
        return;
    dst = src;
    src -= LHLEN(src);
    for (;;) {
        if (LHTYPE(src) == LH_END)
            break;
        if (LHTYPE(src) == LH_FREE) {
            src -= LHLEN(src);
            continue;
        }
        len = LHLEN(src);
        delta = dst - src;
        lh_adjust(src, delta);
        /* upwards, so a copy over itself reads each word before it writes it */
        from = src + 1 - len;
        to = dst + 1 - len;
        for (n = len / 2; n; n--)
            W(to + 2 * (n - 1)) = W(from + 2 * (n - 1));
        src -= len;
        dst -= len;
    }
    if (dst != heap_end)
        set_entry(dst, dst - heap_end, LH_FREE);
    heap_free = dst;
}

/* B$LHADJ with a delta: an entry about to move `delta` bytes up tells what holds its address. */
void lh_adjust(word p, word delta)
{
    word owner, sd, n;

    if (LHTYPE(p) == LH_FILE) {
        if (lh_file_moved)
            lh_file_moved(p, delta);
        return;
    }
    owner = LHBAKP(p);
    if (LHTYPE(p) == LH_ARRAY) {
        sd = p - LHLEN(p) + 3;
        for (n = (LHLEN(p) - LH_STD_HDR - 2) / 4; n; n--, sd += 4)
            str_adjust(sd, delta);
        ((AD *)owner)->adjusted += delta;
    }
    W(owner) += delta;
}

/* B$LHDALC: free an entry by its data pointer.  A dynamic string array frees its strings. */
void lh_free(word data)
{
    word p = lh_entry(data), sd, n;

    if (LHTYPE(p) == LH_ARRAY) {
        ((AD *)LHBAKP(p))->data_seg = 0;
        LHBAKP(p) = 0;
        sd = data;
        for (n = (LHLEN(p) - LH_STD_HDR - 2) / 4; n; n--, sd += 4)
            str_adjust(sd, 0);
    }
    LHTYPE(p) = LH_FREE;
}

/* B$ILHALC with the growth steps of LH_ALC_GROW: the free entry, a scan, then more room taken from
   string space, with string space compacted first when that is not enough. */
word lh_alloc(word bytes, byte type, word owner)
{
    word size = entry_size(bytes, type), data;

    data = alloc_free(size, type, owner);
    if (!data)
        data = scan(size, type, owner);
    if (!data && lh_from_ss())
        data = alloc_free(size, type, owner);
    if (!data) {
        str_compact();
        if (lh_from_ss())
            data = alloc_free(size, type, owner);
    }
    return data;
}

/* B$NHINIT: the dynamic region is all string space until the heap asks for some. */
void nh_init(word first, word last)
{
    heap_first = heap_free = heap_end = last + 1;
    LHTYPE(heap_end) = LH_END;
    nh_first = first;
    nh_last = last;
    str_init(first, last + 1 - (LH_STD_HDR + 1));
}

/* B$xNHINI and B$NHINI: the heaps claim everything from the stack's end to the top of DGROUP. */
extern word qb_atopsp, qb_asizds;

static void nh_ini(void)
{
    nh_init((word)&qb_atopsp, qb_asizds);
}

static Comp nh_comp = { 0, C_NH, { nh_ini } };

#define XI_FN nh_xinit
#include "xi.h"
void nh_xinit(void)
{
    qb_comp_add(&nh_comp);
}
