/* The local heap (QB rt/nhlhcore.asm, nhlhutil.asm) and the dynamic region's
   setup (nhinit.asm). */
#include "nheap.h"
#include "nhstutil.h"
#include "rtinit.h"
#include "startup.h"

char *heap_low;
char *heap_top;

static LhMoved moved[LH_FILE + 1];

enum { HEADER = sizeof(LhEntry), FOOTER = sizeof(uword) };

void lh_on_move(enum LhType type, LhMoved hook)
{
    moved[type] = hook;
}

static LhEntry *lowest(void)
{
    return (LhEntry *)heap_low;
}

static LhEntry *next(const LhEntry *entry)
{
    return (LhEntry *)((char *)entry + entry->size);
}

static LhEntry *previous(const LhEntry *entry)
{
    return (LhEntry *)((char *)entry - ((uword *)entry)[-1]);
}

static int at_top(const LhEntry *entry)
{
    return (char *)entry == heap_top;
}

/* An entry of `size` bytes at `at`, with its footer. */
static LhEntry *make(void *at, uword size, enum LhType type)
{
    LhEntry *entry = at;

    entry->size = size;
    entry->type = type;
    entry->file = 0;
    entry->owner = NULL;
    *(uword *)((char *)at + size - FOOTER) = size;
    return entry;
}

LhEntry *lh_entry(void *data)
{
    return (LhEntry *)data - 1;
}

void *lh_data(LhEntry *entry)
{
    return entry + 1;
}

static uword entry_size(uword bytes)
{
    return (HEADER + bytes + FOOTER + 1) & ~1u;
}

static void zero(LhEntry *entry)
{
    uword *at = lh_data(entry);
    uword *end = (uword *)((char *)entry + entry->size - FOOTER);

    while (at < end)
        *at++ = 0;
}

/* Take `size` bytes from the top of free `entry`, as the heap allocates
   downwards; the rest stays free. */
static void *carve(LhEntry *entry, uword size, enum LhType type)
{
    uword rest = entry->size - size;
    LhEntry *taken;

    if (rest < HEADER + FOOTER) {
        size = entry->size;
        rest = 0;
    }
    taken = (LhEntry *)((char *)entry + rest);
    if (rest)
        make(entry, rest, LH_FREE);
    make(taken, size, type);
    zero(taken);
    return lh_data(taken);
}

/* First fit over the heap, joining free neighbours as it goes. */
static void *first_fit(uword size, enum LhType type)
{
    LhEntry *entry, *after;

    for (entry = lowest(); !at_top(entry); entry = next(entry)) {
        if (entry->type != LH_FREE)
            continue;
        after = next(entry);
        while (!at_top(after) && after->type == LH_FREE) {
            make(entry, entry->size + after->size, LH_FREE);
            after = next(entry);
        }
        if (entry->size >= size)
            return carve(entry, size, type);
    }
    return NULL;
}

int lh_take_from_strings(void)
{
    uword room = str_give_tail();

    if (room < HEADER + FOOTER) {
        str_take(room);
        return 0;
    }
    heap_low -= room;
    make(heap_low, room, LH_FREE);
    if (heap_low + room != heap_top && next(lowest())->type == LH_FREE)
        make(heap_low, room + next(lowest())->size, LH_FREE);
    return 1;
}

/* B$STFromLH */
void lh_give_free_to_strings(void)
{
    while (heap_low != heap_top && lowest()->type == LH_FREE) {
        uword room = lowest()->size;

        heap_low += room;
        str_take(room);
    }
}

/* Slide every entry up against the top, so the free room is one entry at the
   bottom (B$LH_CPCT). */
void lh_compact(void)
{
    char *top = heap_top;
    LhEntry *entry = NULL;

    if (heap_low != heap_top)
        entry = previous((LhEntry *)heap_top);
    while (entry) {
        LhEntry *before = (char *)entry == heap_low ? NULL : previous(entry);

        if (entry->type != LH_FREE) {
            uword *from = (uword *)((char *)entry + entry->size);
            uword *to = (uword *)top;

            top -= entry->size;
            if (top != (char *)entry) {
                if (moved[entry->type])
                    moved[entry->type](lh_data(entry), top - (char *)entry);
                /* from the end down, so a move over itself reads each uword
                   before it writes it */
                while (from > (uword *)entry)
                    *--to = *--from;
            }
        }
        entry = before;
    }
    if (top != heap_low)
        make(heap_low, top - heap_low, LH_FREE);
}

/* An entry of `bytes` of data, for `owner`: the free room, then a scan, then
   room from string space, with string space compacted first when that is not
   enough (LH_ALC_GROW). */
void *lh_alloc(uword bytes, enum LhType type, void *owner, byte file)
{
    uword size = entry_size(bytes);
    void *data = first_fit(size, type);

    if (!data && lh_take_from_strings())
        data = first_fit(size, type);
    if (!data) {
        str_compact();
        if (lh_take_from_strings())
            data = first_fit(size, type);
    }
    if (data) {
        lh_entry(data)->owner = owner;
        lh_entry(data)->file = file;
    }
    return data;
}

void *lh_file(byte channel)
{
    LhEntry *entry;

    for (entry = lowest(); !at_top(entry); entry = next(entry))
        if (entry->type == LH_FILE && (!channel || entry->file == channel))
            return lh_data(entry);
    return NULL;
}

void lh_free(void *data)
{
    lh_entry(data)->type = LH_FREE;
}

/* B$NHINIT: all of the region is string space until the heap asks for some. */
void nh_init(char *first, char *top)
{
    heap_low = heap_top = top;
    str_init(first, top);
}

/* B$xNHINI and B$NHINI: the heaps claim everything from the stack's end to the
   top of DGROUP. */
static void nh_ini(void)
{
    char *first, *top;

    qb_dynamic_region(&first, &top);
    nh_init(first, top);
}

static Comp nh_comp = { 0, C_NH, { nh_ini } };

#define XI_FN nh_xinit
#include "xi.h"
void nh_xinit(void)
{
    qb_comp_add(&nh_comp);
}
