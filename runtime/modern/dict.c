/* A dict's slots (section 13): a vec's descriptor whose length word counts
 * the slots, a power of two, and whose capacity word the entries. A slot is
 * `size` bytes, its hash first; a zero hash marks it free. */

#include "types.h"

/* `table` with room for one more entry, at most three quarters full: a
 * new table of twice the slots, each entry moved by its stored hash. */
char *rt_dict_reserve(char *table, u16 size)
{
    u16 slots = *length_of(table);
    u16 count = *capacity_of(table);
    u16 grown = slots < 8 ? 8 : (u16)(slots * 2);
    u16 mask = (u16)(grown - 1);
    u16 i, at;
    char *copy;
    if ((u16)((count + 1) * 4) <= (u16)(slots * 3))
        return table;
    copy = rt_alloc(grown, size);
    for (i = 0; i < (u16)(grown * size); ++i)
        copy[i] = 0;
    for (i = 0; i < slots; ++i) {
        char *entry = table + i * size;
        u16 hash = *(u16 *)entry;
        if (hash == 0)
            continue;
        at = hash & mask;
        while (*(u16 *)(copy + at * size) != 0)
            at = (u16)((at + 1) & mask);
        copy_bytes(copy + at * size, entry, size);
    }
    *length_of(copy) = grown;
    *capacity_of(copy) = count;
    rt_drop(table);
    return copy;
}

void rt_panic_key(void)
{
    rt_panic("key not found");
}
