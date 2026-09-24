/* The near heap: first fit over one static arena in DGROUP, with the
 * descriptor of section 13 in front of every buffer. */

#include "types.h"

#define ARENA 32768

/* A block is [size][payload]; size counts the header, is even, and its low
 * bit marks the block in use. */
static u16 arena[ARENA / 2];
static u8 ready;

static char *block_alloc(u16 bytes)
{
    char *at = (char *)arena;
    char *end = at + ARENA;
    u16 need = (u16)((bytes + 3) & ~1);
    if (!ready) {
        arena[0] = ARENA;
        ready = 1;
    }
    while (at < end) {
        u16 size = *(u16 *)at;
        if ((size & 1) == 0) {
            while (at + size < end && (*(u16 *)(at + size) & 1) == 0)
                size += *(u16 *)(at + size);
            if (size >= need) {
                if (size - need >= 8) {
                    *(u16 *)(at + need) = size - need;
                    size = need;
                }
                *(u16 *)at = size | 1;
                return at + 2;
            }
            *(u16 *)at = size;
        }
        at += size & ~1;
    }
    rt_panic("out of memory");
    return 0;
}

u8 *flags_of(char *data) { return (u8 *)(data - 6); }
u16 *length_of(char *data) { return (u16 *)(data - 4); }
u16 *capacity_of(char *data) { return (u16 *)(data - 2); }

void copy_bytes(char *to, const char *from, u16 count)
{
    while (count != 0) {
        *to++ = *from++;
        --count;
    }
}

/* An empty heap buffer for `capacity` elements of `size` bytes, and a NUL. */
char *rt_alloc(u16 capacity, u16 size)
{
    char *payload = block_alloc((u16)(6 + capacity * size + 1));
    char *data = payload + 6;
    payload[0] = HEAP;
    payload[1] = 0;
    *length_of(data) = 0;
    *capacity_of(data) = capacity;
    data[0] = 0;
    return data;
}

/* `data` on the heap, writable, with room for `capacity` elements: the same
 * buffer when it already is, else a copy. The old buffer is freed. */
char *rt_reserve(char *data, u16 capacity, u16 size)
{
    char *copy;
    u16 length = *length_of(data);
    u16 room = *capacity_of(data);
    if (capacity < length)
        capacity = length;
    if ((*flags_of(data) & (HEAP | READONLY)) == HEAP && room >= capacity)
        return data;
    if (capacity < 2 * room)
        capacity = 2 * room;
    copy = rt_alloc(capacity, size);
    copy_bytes(copy, data, (u16)(length * size + 1));
    *length_of(copy) = length;
    rt_drop(data);
    return copy;
}

void rt_drop(char *data)
{
    if (data != 0 && (*flags_of(data) & HEAP) != 0)
        *(u16 *)(data - 8) &= ~1;
}
