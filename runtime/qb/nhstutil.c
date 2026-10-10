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

/* A string takes whole words, so the entries (and the owner pointers in their headers) stay aligned. */
static uword even(uword n)
{
    return (n + WORD - 1) & ~(uword)(WORD - 1);
}

static int is_free(const StrEntry *entry)
{
    return entry->header & 1;
}

static uword free_data(const StrEntry *entry)
{
    return entry->header - 1;
}

static SD *owner_of(const StrEntry *entry)
{
    return (SD *)entry->header;
}

static char *data_of(StrEntry *entry)
{
    return (char *)(entry + 1);
}

static StrEntry *entry_of(const char *data)
{
    return (StrEntry *)data - 1;
}

static uword entry_bytes(const StrEntry *entry)
{
    uword data = is_free(entry) ? free_data(entry) : even(owner_of(entry)->len);

    return WORD + data;
}

static StrEntry *following(const StrEntry *entry)
{
    return (StrEntry *)((char *)entry + entry_bytes(entry));
}

static int at_end(const StrEntry *entry)
{
    return (char *)entry >= str_end;
}

static void make_free(StrEntry *entry, uword bytes)
{
    entry->header = bytes - WORD + 1;
}

/* A string of `len` bytes is in string space when its data is between the
   constants and the boundary. */
static int in_space(const SD *sd)
{
    return sd->len && sd->ptr >= (char *)str_first && sd->ptr < str_end;
}

static void check_owner(const StrEntry *entry, const SD *owner)
{
    if (owner_of(entry) != owner || owner->ptr != (char *)(entry + 1))
        corrupt();
}

/* B$STSetFree: the hint is kept only if it names the free entry that ends
   string space. */
static void set_hint(void)
{
    if (at_end(str_free) || !is_free(str_free) || !at_end(following(str_free)))
        str_free = (StrEntry *)str_end;
}

/* Takes `bytes` for `owner` from free `entry`, leaving the rest free. */
static char *take(StrEntry *entry, uword bytes, SD *owner)
{
    uword have = entry_bytes(entry);

    if (have > bytes) {
        make_free((StrEntry *)((char *)entry + bytes), have - bytes);
        str_free = (StrEntry *)((char *)entry + bytes);
    } else {
        str_free = following(entry);
    }
    entry->header = (uword)owner;
    return data_of(entry);
}

/* First fit from `from` up to `limit`, joining free neighbours as it goes
   (SS_SCAN). */
static StrEntry *scan(StrEntry *from, StrEntry *limit, uword bytes)
{
    StrEntry *entry = from, *after;

    while (entry <= limit && !at_end(entry)) {
        if (is_free(entry)) {
            after = following(entry);
            while (!at_end(after) && is_free(after)) {
                make_free(entry, entry_bytes(entry) + entry_bytes(after));
                after = following(entry);
            }
            if (entry_bytes(entry) >= bytes)
                return entry;
        }
        entry = following(entry);
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
    return at_end(str_free) ? 0 : free_data(str_free);
}

/* B$STCPCT: slide every string down over the free entries, so the free room is
   one entry at the end. */
void str_compact(void)
{
    StrEntry *to = str_first, *entry = str_first;

    while (!at_end(entry)) {
        uword bytes = entry_bytes(entry), i;
        StrEntry *after = (StrEntry *)((char *)entry + bytes);

        if (!is_free(entry)) {
            SD *owner = owner_of(entry);

            check_owner(entry, owner);
            if (to != entry)
                for (i = 0; i < bytes; i += WORD)
                    *(uword *)((char *)to + i) = *(uword *)((char *)entry + i);
            owner->ptr = data_of(to);
            to = (StrEntry *)((char *)to + bytes);
        }
        entry = after;
    }
    if ((char *)to != str_end)
        make_free(to, str_end - (char *)to);
    str_free = to;
}

/* The free entry that ends string space, which the heap may take: its size, and
   the boundary moves up by as much as is given. */
uword str_give_tail(void)
{
    uword room;

    set_hint();
    if (at_end(str_free))
        return 0;
    room = entry_bytes(str_free);
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
    if (at_end(str_free)) {
        str_free = (StrEntry *)str_end;
        make_free(str_free, bytes);
    } else {
        make_free(str_free, entry_bytes(str_free) + bytes);
    }
    str_end += bytes;
}

/* B$STALC: room for `owner`'s string.  Out of room, it tries a scan, the heap's
   free room, then compaction (nhstutil.asm:161-215), and raises Out of string
   space if there is still none. */
char *str_alloc(SD *owner, uword len)
{
    uword bytes = WORD + even(len);
    StrEntry *entry;

    if (len > SD_MAX_LENGTH)
        qb_error(BE_STRINGSP);
    owner->len = len;
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
        qb_error(BE_STRINGSP);
    owner->ptr = take(entry, bytes, owner);
    return owner->ptr;
}

void str_release(SD *owner)
{
    StrEntry *entry;

    if (!in_space(owner))
        return;
    entry = entry_of(owner->ptr);
    check_owner(entry, owner);
    make_free(entry, entry_bytes(entry));
}

void str_owner_moved(SD *owner, int delta)
{
    if (in_space(owner))
        entry_of(owner->ptr)->header += delta;
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
        entry_of(to->ptr)->header = (uword)to;
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
    make_free(str_first, end - first);
    cur_level = 0;
    str_nul.len = 0;
    str_nul.ptr = &nul_owner_slot;
    for (tmp = temps; tmp < temps + NUMTEMPS; tmp++)
        tmp_release(tmp);
}
