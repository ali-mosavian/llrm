/* String space and string temporaries (QB rt/nhstutil.asm).  See nhstutil.h for
   the layout. */
#include "nhstutil.h"

/* An entry is a header of one word, then its data; WORD is that word, QB's two bytes where a near pointer is two. */
enum { NUMTEMPS = 20, WORD = sizeof(uword) };

typedef struct StrEntry {
    uword header;
} StrEntry;

/* A string temporary (inc/string.inc LenTemp = 6): a descriptor, and the
   program level that made it or -1 while it is free. */
typedef struct Tmp {
    SD sd;
    int level;
} Tmp;

static StrEntry *str_first;
static StrEntry *str_free;   /* where the last allocation left off: a hint */
static char *str_end;

int cur_level;
SD str_nul;

static Tmp temps[NUMTEMPS];
static byte nul_owner_slot;

/* B$ERR_SSC */
static void corrupt(void)
{
    qb_error(FE_CORRUPT);
}

/* The entry helpers are macros: each is a line or two, and a call costs more than they do on every string. */

/* A string takes whole words, so the entries (and the owner pointers in their headers) stay aligned. */
#define EVEN(n) (((n) + WORD - 1) & ~(uword)(WORD - 1))
#define IS_FREE(entry) ((entry)->header & 1)
#define FREE_DATA(entry) ((entry)->header - 1)
#define OWNER_OF(entry) ((SD *)(entry)->header)
#define DATA_OF(entry) ((char *)((entry) + 1))
#define ENTRY_OF(data) ((StrEntry *)(data) - 1)
#define ENTRY_BYTES(entry) (WORD + (IS_FREE(entry) ? FREE_DATA(entry) : EVEN(OWNER_OF(entry)->len)))
#define FOLLOWING(entry) ((StrEntry *)((char *)(entry) + ENTRY_BYTES(entry)))
#define AT_END(entry) ((char *)(entry) >= str_end)
#define MAKE_FREE(entry, bytes) ((entry)->header = (bytes) - WORD + 1)

/* A string of `len` bytes is in string space when its data is between the
   constants and the boundary. */
#define IN_SPACE(sd) ((sd)->len && (sd)->ptr >= (char *)str_first && (sd)->ptr < str_end)

#define CHECK_OWNER(entry, owner) \
    do { \
        if (OWNER_OF(entry) != (owner) || (owner)->ptr != (char *)((entry) + 1)) \
            corrupt(); \
    } while (0)

/* B$STSetFree: the hint is kept only if it names the free entry that ends
   string space. */
static void set_hint(void)
{
    if (AT_END(str_free) || !IS_FREE(str_free) || !AT_END(FOLLOWING(str_free)))
        str_free = (StrEntry *)str_end;
}

/* Takes `bytes` for `owner` from free `entry`, leaving the rest free. */
static char *take(StrEntry *entry, uword bytes, SD *owner)
{
    uword have = ENTRY_BYTES(entry);

    if (have > bytes) {
        MAKE_FREE((StrEntry *)((char *)entry + bytes), have - bytes);
        str_free = (StrEntry *)((char *)entry + bytes);
    } else {
        str_free = FOLLOWING(entry);
    }
    entry->header = (uword)owner;
    return DATA_OF(entry);
}

/* First fit from `from` up to `limit`, joining free neighbours as it goes
   (SS_SCAN). */
static StrEntry *scan(StrEntry *from, StrEntry *limit, uword bytes)
{
    StrEntry *entry = from, *after;

    while (entry <= limit && !AT_END(entry)) {
        if (IS_FREE(entry)) {
            after = FOLLOWING(entry);
            while (!AT_END(after) && IS_FREE(after)) {
                MAKE_FREE(entry, ENTRY_BYTES(entry) + ENTRY_BYTES(after));
                after = FOLLOWING(entry);
            }
            if (ENTRY_BYTES(entry) >= bytes)
                return entry;
        }
        entry = FOLLOWING(entry);
    }
    return NULL;
}

static StrEntry *fit(uword bytes)
{
    StrEntry *found = scan(str_free, (StrEntry *)str_end, bytes);

    if (!found)
        found = scan(str_first, str_free, bytes);
    return found;
}

/* The bytes of string space nothing holds, once the strings are packed. */
uword str_free_bytes(void)
{
    str_compact();
    return AT_END(str_free) ? 0 : FREE_DATA(str_free);
}

/* B$STCPCT: slide every string down over the free entries, so the free room is
   one entry at the end. */
void str_compact(void)
{
    StrEntry *to = str_first, *entry = str_first;

    while (!AT_END(entry)) {
        uword bytes = ENTRY_BYTES(entry), i;
        StrEntry *after = (StrEntry *)((char *)entry + bytes);

        if (!IS_FREE(entry)) {
            SD *owner = OWNER_OF(entry);

            CHECK_OWNER(entry, owner);
            if (to != entry)
                for (i = 0; i < bytes; i += WORD)
                    *(uword *)((char *)to + i) = *(uword *)((char *)entry + i);
            owner->ptr = DATA_OF(to);
            to = (StrEntry *)((char *)to + bytes);
        }
        entry = after;
    }
    if ((char *)to != str_end)
        MAKE_FREE(to, str_end - (char *)to);
    str_free = to;
}

/* The free entry that ends string space, which the heap may take: its size, and
   the boundary moves up by as much as is given. */
uword str_give_tail(void)
{
    uword room;

    set_hint();
    if (AT_END(str_free))
        return 0;
    room = ENTRY_BYTES(str_free);
    str_end = (char *)str_free;
    str_free = (StrEntry *)str_end;
    return room;
}

/* The heap's free room at the boundary becomes the end of string space. */
void str_take(uword bytes)
{
    if (!bytes)
        return;
    set_hint();
    if (AT_END(str_free)) {
        str_free = (StrEntry *)str_end;
        MAKE_FREE(str_free, bytes);
    } else {
        MAKE_FREE(str_free, ENTRY_BYTES(str_free) + bytes);
    }
    str_end += bytes;
}

/* B$STALC: room for `owner`'s string.  Out of room, it tries a scan, the heap's
   free room, then compaction (nhstutil.asm:161-215), and raises Out of string
   space if there is still none. */
char *str_alloc(SD *owner, uword len)
{
    uword bytes = WORD + EVEN(len);
    StrEntry *entry = str_free;

    if (len > SD_MAX_LENGTH)
        goto none;
    owner->len = len;
    /* The usual case: the hint is a free entry with room. */
    if (!AT_END(entry) && IS_FREE(entry) && WORD + FREE_DATA(entry) >= bytes)
        goto found;
    entry = fit(bytes);
    if (!entry) {
        lh_give_free_to_strings();
        entry = fit(bytes);
    }
    if (!entry) {
        str_compact();
        entry = fit(bytes);
    }
    if (!entry)
        goto none;
found:
    owner->ptr = take(entry, bytes, owner);
    return owner->ptr;
none:
    /* The descriptor holds no string, so nothing later frees what is not there. */
    owner->len = 0;
    owner->ptr = &nul_owner_slot;
    qb_error(BE_STRINGSP);
    return NULL;
}

void str_release(SD *owner)
{
    StrEntry *entry;
    uword bytes;

    if (!IN_SPACE(owner))
        return;
    entry = ENTRY_OF(owner->ptr);
    CHECK_OWNER(entry, owner);
    bytes = WORD + EVEN(owner->len);
    MAKE_FREE(entry, bytes);
}

void str_owner_moved(SD *owner, int delta)
{
    if (IN_SPACE(owner))
        ENTRY_OF(owner->ptr)->header += delta;
}

static Tmp *as_tmp(SD *sd)
{
    return (Tmp *)sd;
}

byte str_is_tmp(const SD *sd)
{
    return (const Tmp *)sd >= temps && (const Tmp *)sd < temps + NUMTEMPS;
}

static Tmp *free_tmp(void)
{
    Tmp *tmp;

    for (tmp = temps; tmp < temps + NUMTEMPS; tmp++)
        if (tmp->level < 0)
            return tmp;
    qb_error(BE_STRINGFO);
    return NULL;
}

/* B$STDALCTMPDSC: the descriptor is free again, the string untouched. */
static void tmp_release(Tmp *tmp)
{
    tmp->level = -1;
}

void str_adopt(SD *to, SD *from)
{
    str_release(to);
    *to = *from;
    if (to->len)
        ENTRY_OF(to->ptr)->header = (uword)to;
    if (str_is_tmp(from))
        tmp_release(as_tmp(from));
}

/* B$STDALCTMP: a temporary's string and descriptor, freed; anything else is
   left. */
void str_tmp_free(SD *sd)
{
    if (str_is_tmp(sd)) {
        str_release(sd);
        tmp_release(as_tmp(sd));
    }
}

/* B$STALCTMP: a temporary of `len` bytes, its data in *data.  Zero bytes is the
   shared empty string. */
SD *str_tmp(uword len, char **data)
{
    Tmp *tmp;

    if (len == 0) {
        *data = str_nul.ptr;
        return &str_nul;
    }
    tmp = free_tmp();
    *data = str_alloc(&tmp->sd, len);
    tmp->level = cur_level;
    return &tmp->sd;
}

/* B$STALCTMPSUB: a temporary copy of `len` bytes of source from `from`; a
   temporary source is freed. */
SD *str_tmp_copy(SD *source, uword from, uword len)
{
    char *data;
    SD *tmp = str_tmp(len, &data);

    copy_bytes(data, source->ptr + from, len);
    str_tmp_free(source);
    return tmp;
}

/* B$STDALCALLTMP: the temporaries made at program level `level` or deeper are
   freed. */
void str_all_tmp_free(int level)
{
    Tmp *tmp;

    for (tmp = temps; tmp < temps + NUMTEMPS; tmp++)
        if (tmp->level >= level)
            str_tmp_free(&tmp->sd);
}

/* B$STINIT: all of string space is one free entry, and every temporary is free.
   */
void str_init(char *first, char *end)
{
    Tmp *tmp;

    str_first = str_free = (StrEntry *)first;
    str_end = end;
    MAKE_FREE(str_first, end - first);
    cur_level = 0;
    str_nul.len = 0;
    str_nul.ptr = &nul_owner_slot;
    for (tmp = temps; tmp < temps + NUMTEMPS; tmp++)
        tmp_release(tmp);
}
