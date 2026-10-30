/* The far heap (QB rt/fhinit.asm). */
#include "far.h"
#include "fheap.h"
#include "nheap.h"
#include "rtinit.h"
#include "shell.h"
#include "llrm_os.h"

#pragma aux llrm_os_psp "LL$PSP"

extern u16 llrm_os_psp;

/* The list's ends: the top, from which entries are placed downwards, and the
   bottom, above DGROUP. */
static FHD top, bottom;

static FHD *next_of(const FHD *entry)
{
    return (FHD *)entry->next;
}

/* The paragraphs of free room just below `entry`, before the one that follows
   it. */
static u16 room_below(const FHD *entry)
{
    const FHD *next = next_of(entry);

    return entry->data_seg - (next->data_seg + next->size);
}

static int place(FHD *after, FHD *owner, u16 paras)
{
    FHD *next = next_of(after);

    if (room_below(after) < paras)
        return 0;
    owner->data_seg = after->data_seg - paras;
    owner->size = paras;
    owner->next = (u16)next;
    after->next = (u16)owner;
    far_clear(owner->data_seg, paras);
    return 1;
}

static int place_anywhere(FHD *owner, u16 paras)
{
    FHD *entry;

    for (entry = &top; entry != &bottom; entry = next_of(entry))
        if (place(entry, owner, paras))
            return 1;
    return 0;
}

/* B$FHCompact: slide every entry up against the top, joining the free room into
   one piece at the bottom. */
static void compact(void)
{
    u16 at = top.data_seg;
    FHD *entry;

    for (entry = next_of(&top); entry != &bottom; entry = next_of(entry)) {
        at -= entry->size;
        if (entry->data_seg != at) {
            far_move(entry->data_seg, at, entry->size);
            entry->data_seg = at;
        }
    }
}

void fh_alloc(FHD *owner, unsigned long bytes)
{
    unsigned long paras = (bytes + 15) >> 4;

    if (paras <= 0xFFFF) {
        if (place_anywhere(owner, paras))
            return;
        compact();
        if (place_anywhere(owner, paras))
            return;
    }
    qb_error(BE_MEMORY);
}

/* B$FHDealloc: unlink the entry; its descriptor no longer names memory. */
void fh_free(FHD *owner)
{
    FHD *entry = &top;

    while (next_of(entry) != owner && entry != &bottom)
        entry = next_of(entry);
    if (entry == &bottom)
        qb_error(FE_CORRUPT);
    entry->next = owner->next;
    owner->data_seg = 0;
}

/* `to` takes `from`'s place in the list and its paragraphs; `from` is left without. */
void fh_adopt(FHD *to, FHD *from)
{
    FHD *entry = &top;

    while (next_of(entry) != from && entry != &bottom)
        entry = next_of(entry);
    if (entry == &bottom)
        qb_error(FE_CORRUPT);
    entry->next = (u16)to;
    to->next = from->next;
    to->data_seg = from->data_seg;
    to->size = from->size;
    from->data_seg = 0;
}

/* The bytes of far heap nothing holds. */
unsigned long fh_free_bytes(void)
{
    const FHD *entry;
    unsigned long paras = 0;

    for (entry = &top; entry != &bottom; entry = next_of(entry))
        paras += room_below(entry);
    return paras << 4;
}

/* The paragraph just past the program's last array once every entry is slid down against the bottom, lowest first. */
static u16 pack_down(void)
{
    u16 at = bottom.data_seg + bottom.size;
    int moved = 1;

    while (moved) {
        FHD *entry, *lowest = 0;

        moved = 0;
        for (entry = next_of(&top); entry != &bottom; entry = next_of(entry))
            if (entry->data_seg >= at && (!lowest || entry->data_seg < lowest->data_seg))
                lowest = entry;
        if (lowest) {
            if (lowest->data_seg != at)
                far_move(lowest->data_seg, at, lowest->size);
            lowest->data_seg = at;
            at += lowest->size;
            moved = 1;
        }
    }
    return at;
}

/* SHELL: the arrays slide down and the DOS block shrinks to them, which leaves the child the rest. */
void qb_shell_release(void)
{
    u16 end = pack_down();

    top.data_seg = end;
    llrm_os_block_resize(llrm_os_psp, end - llrm_os_psp);
}

/* After the child: the whole block again, the arrays slid back up against its top. */
void qb_shell_reclaim(void)
{
    u16 block = llrm_os_block_resize(llrm_os_psp, 0xFFFF);

    top.data_seg = llrm_os_psp + block;
    compact();
}

/* The paragraphs DGROUP takes, up to the top of the local heap. */
static u16 heap_paragraphs(void)
{
    return ((unsigned long)heap_top + 15) >> 4;
}

/* B$FHIni: take the program's whole DOS block, and let the heap be what DGROUP
   leaves of it. */
static void fh_ini(void)
{
    u16 block = llrm_os_block_resize(llrm_os_psp, 0xFFFF);

    top.data_seg = llrm_os_psp + block;
    top.size = 0;
    top.next = (u16)&bottom;
    bottom.data_seg = dgroup_segment() + heap_paragraphs();
    bottom.size = 0;
    bottom.next = 0;
}

static Comp fh_comp = {0, C_FH, {fh_ini}};

#define XI_FN fh_xinit
#include "xi.h"

void fh_xinit(void)
{
    qb_comp_add(&fh_comp);
}
