/* String space and string temporaries (QB rt/nhstutil.asm).  See nheap.h for the entry format. */
#include "nhstutil.h"

enum { NUMTEMPS = 20 };

/* A string temporary (inc/string.inc LenTemp = 6): a descriptor and the program level that made it.
   While free, `len` links to the next free one and `level` is -1. */
typedef struct Tmp {
    word len;
    word ptr;
    int level;
} Tmp;

word str_first, str_free, str_end;
word cur_level;
SD str_nul;

static word nul_back;
static Tmp temps[NUMTEMPS];
static Tmp *tmp_head;

/* B$ERR_SSC */
static void corrupt(void)
{
    qb_error(FE_CORRUPT);
}

static word entry_len(word sd)
{
    return (W(sd) + 3) & ~1u;
}

/* B$STSetFree: str_free is only a hint; keep it only if it names the free entry that ends the space. */
void str_set_free(void)
{
    word h = W(str_free);

    if ((h & 1) && (h == 0xFFFF || str_free + h + 1 == str_end))
        return;
    str_free = str_end;
}

/* SS_ALC: take hdr out of the free entry at `at`, whose header is h >= hdr. */
static word take(word at, word h, word hdr)
{
    if (h == hdr) {
        str_free = at + h + 1;
    } else {
        W(at) = hdr;
        str_free = at + hdr + 1;
        W(str_free) = h - hdr - 1;
    }
    return at + 2;
}

/* SS_SCAN_START: first fit from `si` while si <= to, joining free neighbours as it goes. */
static word scan(word hdr, word si, word to, word *stop)
{
    word h, c, di;

    for (;;) {
        h = W(si);
        if (h & 1) {
            if (h == 0xFFFF)
                break;
            for (di = si + h + 1;; di += c + 1) {
                c = W(di);
                if (c == 0xFFFF || !(c & 1)) {
                    W(si) = h;
                    if (h >= hdr) {
                        str_free = si;
                        return take(si, h, hdr);
                    }
                    if (c == 0xFFFF) {
                        *stop = si;
                        return 0;
                    }
                    si = di;
                    h = c;
                    break;
                }
                h += c + 1;
            }
        }
        si += entry_len(h);
        if (si > to)
            break;
    }
    *stop = si;
    return 0;
}

/* SS_SCAN: from the free hint to the end, then from the start up to the hint. */
static word scan_both(word hdr)
{
    word stop, stop2, at;

    at = scan(hdr, str_free, str_end, &stop);
    if (at)
        return at;
    at = scan(hdr, str_first, str_free, &stop2);
    if (at)
        return at;
    str_free = stop2 < stop ? stop2 : stop;
    str_set_free();
    return 0;
}

/* B$STCPCT: slide every string down over the free entries, fixing the descriptors, so the free space
   is one entry at the end. */
void str_compact(void)
{
    word si = str_first, di, h, n;

    for (h = W(si); !(h & 1); h = W(si)) {
        if (W(h + 2) - 2 != si)
            corrupt();
        si += entry_len(h);
    }
    di = si;
    for (;;) {
        if (h == 0xFFFF)
            break;
        si += h + 1;
        for (h = W(si); !(h & 1); h = W(si)) {
            if (W(h + 2) - 2 != si)
                corrupt();
            W(h + 2) = di + 2;
            for (n = entry_len(h) / 2; n; n--, si += 2, di += 2)
                W(di) = W(si);
        }
    }
    if (si != di)
        W(di) = si - di - 1;
    str_free = di;
}

/* B$STFromLH: free entries at the heap's end become string space. */
void str_from_lh(void)
{
    word end, at;

    lh_set_free();
    if (heap_free == heap_end)
        return;
    heap_end = heap_free;
    LHTYPE(heap_end) = LH_END;
    end = heap_end - (LH_STD_HDR + 1);
    W(end) = 0xFFFF;
    str_set_free();
    at = str_free;
    str_end = end;
    W(at) = end - at - 1;
}

/* B$STALC: the data of a new string of `len` bytes; the caller sets the back-pointer at data - 2.
   Out of room, it tries the free entry, a scan, the heap's free space, then compaction. */
word str_alloc(word len)
{
    word hdr, h, data;

    if (len == 0xFFFF)
        qb_error(BE_STRINGSP);
    hdr = (len + 1) | 1;
    h = W(str_free);
    if ((h & 1) && h != 0xFFFF && hdr <= h)
        return take(str_free, h, hdr);
    data = scan_both(hdr);
    if (data)
        return data;
    str_from_lh();
    h = W(str_free);
    if ((h & 1) && h != 0xFFFF && hdr <= h)
        return take(str_free, h, hdr);
    str_compact();
    h = W(str_free);
    if ((h & 1) && h != 0xFFFF && hdr <= h)
        return take(str_free, h, hdr);
    qb_error(BE_STRINGSP);
    return 0;
}

/* B$STADJ: a descriptor moved `delta` bytes (its string's back-pointer follows), or with delta 0 is
   deleted (its string freed).  Constants lie below string space; above it are fielded strings. */
void str_adjust(word sd, word delta)
{
    word data = ((SD *)sd)->ptr, old;

    if (((SD *)sd)->len == 0 || data < str_first || data > str_end)
        return;
    if (delta) {
        W(data - 2) += delta;
        return;
    }
    old = W(data - 2);
    W(data - 2) = (((SD *)sd)->len + 1) | 1;
    if (old != sd)
        corrupt();
}

void str_free_sd(SD *sd)
{
    str_adjust((word)sd, 0);
}

static byte is_tmp(SD *sd)
{
    return (word)sd >= (word)temps && (word)sd < (word)(temps + NUMTEMPS);
}

static void tmp_release(Tmp *t)
{
    t->len = (word)tmp_head;
    t->ptr = 0xFFFF;
    t->level = -1;
    tmp_head = t;
}

/* B$STDALCTMPDSC: give a temporary descriptor back without touching its string. */
void str_tmp_release(SD *sd)
{
    if (is_tmp(sd))
        tmp_release((Tmp *)sd);
}

/* B$STCHKTMP */
byte str_is_tmp(SD *sd)
{
    return is_tmp(sd);
}

/* B$STDALCTMP: free a string if it is a temporary. */
void str_tmp_free(SD *sd)
{
    if (is_tmp(sd)) {
        str_free_sd(sd);
        tmp_release((Tmp *)sd);
    }
}

/* B$STALCTMP: a temporary of `len` bytes; its data in *data.  A zero length is the shared null string. */
SD *str_tmp(word len, word *data)
{
    Tmp *t;

    if (len == 0) {
        *data = str_nul.ptr;
        return &str_nul;
    }
    if (!tmp_head)
        qb_error(BE_STRINGFO);
    *data = str_alloc(len);
    t = tmp_head;
    tmp_head = (Tmp *)t->len;
    t->len = len;
    t->ptr = *data;
    t->level = cur_level;
    W(*data - 2) = (word)t;
    return (SD *)t;
}

/* B$STALCTMPSUB: a temporary copy of `len` bytes of src from `off`; a temporary src is freed. */
SD *str_tmp_sub(SD *src, word off, word len)
{
    word data, n;
    SD *t = str_tmp(len, &data);
    byte *from = (byte *)(src->ptr + off);

    for (n = 0; n < len; n++)
        B(data + n) = from[n];
    str_tmp_free(src);
    return t;
}

/* B$STDALCALLTMP: free the temporaries made at program level `level` or deeper. */
void str_all_tmp_free(word level)
{
    Tmp *t;

    for (t = temps; t < temps + NUMTEMPS; t++)
        if (t->level >= (int)level)
            str_tmp_free((SD *)t);
}

/* B$STINIT: all of the dynamic region is string space, and every temporary is free. */
void str_init(word first, word end)
{
    word n;

    str_first = str_free = first;
    str_end = end;
    W(end) = 0xFFFF;
    W(first) = end - first - 1;
    cur_level = 0;
    nul_back = (word)&str_nul;
    str_nul.len = 0;
    str_nul.ptr = (word)&nul_back + 2;
    tmp_head = 0;
    for (n = NUMTEMPS; n; n--)
        tmp_release(&temps[n - 1]);
}
