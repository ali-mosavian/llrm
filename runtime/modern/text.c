/* Owned strings: building, joining, and comparing (section 13). */

#include "types.h"

/* `text` with `count` more bytes after it; it may move. */
char *rt_append_bytes(char *text, const char *bytes, u16 count)
{
    u16 length = *length_of(text);
    text = rt_reserve(text, (u16)(length + count), 1);
    copy_bytes(text + length, bytes, count);
    length += count;
    *length_of(text) = length;
    text[length] = 0;
    return text;
}

char *rt_append(char *text, char *more)
{
    return rt_append_bytes(text, more, *length_of(more));
}

char *rt_concat(char *left, char *right)
{
    u16 length = *length_of(left);
    char *joined = rt_alloc((u16)(length + *length_of(right)), 1);
    copy_bytes(joined, left, length);
    *length_of(joined) = length;
    return rt_append(joined, right);
}

/* An owned copy of a `&string` view. */
char *rt_view_copy(const char far *data, u16 length)
{
    char *copy = rt_alloc(length, 1);
    u16 at;
    for (at = 0; at < length; ++at)
        copy[at] = data[at];
    copy[length] = 0;
    *length_of(copy) = length;
    return copy;
}

/* -1, 0, or 1 as the view `left` sorts before, with, or after `right`. */
i8 rt_view_compare(const char far *left, u16 left_length, const char far *right, u16 right_length)
{
    u16 at;
    for (at = 0; at < left_length && at < right_length; ++at) {
        if (left[at] != right[at])
            return (u8)left[at] < (u8)right[at] ? -1 : 1;
    }
    if (left_length == right_length)
        return 0;
    return left_length < right_length ? -1 : 1;
}

/* -1, 0, or 1 as `left` sorts before, with, or after `right`. */
i8 rt_compare(char *left, char *right)
{
    return rt_view_compare(left, *length_of(left), right, *length_of(right));
}
