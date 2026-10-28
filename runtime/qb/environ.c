/* ENVIRON name=value (QB rt/osstmt.asm B$SENV): the program's environment block is edited in place, as DOS
   keeps it. */
#include "llrm_os.h"
#include "nhstutil.h"
#include "qb.h"

enum { SPEC_MAX = 255, CHUNK = 64 };

/* QB's environment table is the one the program started with (rt/osinit.asm B$OSINI: b$env_len is its end): a
   variable goes in only where the table has shrunk. Zero until the first ENVIRON measures it. */
static unsigned table_limit;

static unsigned env_read(unsigned offset, char *data, unsigned max)
{
    return llrm_os_environment_read(offset, (os_data_mut)data, max);
}

static void env_write(unsigned offset, const char *data, unsigned count)
{
    if (llrm_os_environment_write(offset, (os_data)data, count) < 0)
        qb_error(BE_MEMORY);
}

/* The length of the NUL-ended string at `offset`; the block's end is a string of none. */
static unsigned string_length(unsigned offset)
{
    char chunk[CHUNK];
    unsigned length = 0, got, at;

    for (;;) {
        got = env_read(offset + length, chunk, CHUNK);
        for (at = 0; at < got; at++)
            if (chunk[at] == 0)
                return length + at;
        if (got == 0)
            return length;
        length += got;
    }
}

/* The bytes from `from` on, `count` of them, moved down to `to`. */
static void move_down(unsigned to, unsigned from, unsigned count)
{
    char chunk[CHUNK];
    unsigned got;

    while (count) {
        got = env_read(from, chunk, count < CHUNK ? count : CHUNK);
        if (got == 0)
            return;
        env_write(to, chunk, got);
        to += got;
        from += got;
        count -= got;
    }
}

#pragma aux B_SENV "B$SENV"

/* B$SENV: ENVIRON "name=value" or "name value". An existing name is replaced; no value, or a lone ';', removes
   it. No delimiter or an empty name is Illegal function call; no room is Out of memory. */
void B_SENV(SD *spec)
{
    char text[SPEC_MAX], head[CHUNK + 1], entry[SPEC_MAX + 2];
    unsigned n = spec->len, name, value, scan, offset = 0, length, found = 0, found_length = 0, at;
    int remove;

    if (n == 0 || n > SPEC_MAX) {
        str_tmp_free(spec);
        qb_error(BE_ILLFUN);
    }
    copy_bytes(text, spec->ptr, n);
    str_tmp_free(spec);
    for (name = 0; name < n && text[name] != ' ' && text[name] != '='; name++)
        ;
    if (name == 0 || name == n)
        qb_error(BE_ILLFUN);
    value = name + 1;
    if (text[name] == ' ') {
        while (value < n && text[value] == ' ')
            value++;
        if (value < n && text[value] == '=')
            value++;
    }
    scan = value;
    while (scan < n && text[scan] == ' ')
        scan++;
    remove = scan >= n || (n - scan == 1 && text[scan] == ';');

    for (;;) {
        length = string_length(offset);
        if (length == 0)
            break;
        if (!found && length > name && env_read(offset, head, name + 1) == name + 1 && head[name] == '=') {
            for (at = 0; at < name && head[at] == text[at]; at++)
                ;
            if (at == name) {
                found = 1;
                found_length = length;
                scan = offset;
            }
        }
        offset += length + 1;
    }
    if (table_limit == 0)
        table_limit = offset;
    if (found) {
        move_down(scan, scan + found_length + 1, offset + 1 - (scan + found_length + 1));
        offset -= found_length + 1;
    }
    if (remove)
        return;
    if (offset + n >= table_limit)
        qb_error(BE_MEMORY);
    copy_bytes(entry, text, name);
    entry[name] = '=';
    copy_bytes(entry + name + 1, text + value, n - value);
    length = name + 1 + (n - value);
    entry[length] = 0;
    entry[length + 1] = 0;
    env_write(offset, entry, length + 2);
}
