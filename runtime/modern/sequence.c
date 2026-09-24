/* Growing, shrinking, and copying a string or vec of `size`-byte elements
 * (section 13). */

#include "types.h"

/* `data` with `count` more elements counted in its length; it may move. */
char *rt_grow(char *data, u16 count, u16 size)
{
    u16 length = *length_of(data);
    data = rt_reserve(data, (u16)(length + count), size);
    *length_of(data) = (u16)(length + count);
    return data;
}

/* `data` without its last `count` elements; the new length. */
u16 rt_shrink(char *data, u16 count)
{
    u16 length = *length_of(data);
    if (length < count)
        rt_panic("pop from an empty vec");
    length -= count;
    *length_of(data) = length;
    return length;
}

/* A heap copy of `data`. */
char *rt_clone(char *data, u16 size)
{
    u16 length = *length_of(data);
    char *copy = rt_alloc(length, size);
    copy_bytes(copy, data, (u16)(length * size + 1));
    *length_of(copy) = length;
    return copy;
}
