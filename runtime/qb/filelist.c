/* FILES (QB rt/dkstmt.asm B$FILS): the current directory, the files and directories that match, three of the
   18-character entries to the line a screen of 80 columns holds, and the free space. */
#include "console.h"
#include "llrm_os.h"
#include "nhstutil.h"
#include "qb.h"

enum { PATH_MAX = 128, ATTR_SUBDIR = 0x10, ENTRY_BYTES = 43, ENTRY_ATTR = 21, ENTRY_NAME = 30, ENTRY_WIDTH = 18 };

static void put_text(const char *text)
{
    for (; *text; text++)
        cn_putc(*text);
}

/* A number as PRINT writes it: a space for the sign, the digits, a space. */
static void put_number(long value)
{
    char digits[12];
    int count = 0;

    cn_putc(value < 0 ? '-' : ' ');
    if (value < 0)
        value = -value;
    do
        digits[count++] = '0' + value % 10;
    while ((value /= 10) != 0);
    while (count)
        cn_putc(digits[--count]);
    cn_putc(' ');
}

/* One entry as B$FILS writes it: the name's first 8 characters up to a dot, then 4 from the dot, then 6 for <DIR>. */
static void put_entry(const unsigned char *entry)
{
    const char *at = (const char *)entry + ENTRY_NAME;
    int count;

    for (count = 0; count < 8; count++)
        cn_putc(*at == 0 || *at == '.' ? ' ' : *at++);
    for (count = 0; count < 4; count++)
        cn_putc(*at == 0 ? ' ' : *at++);
    put_text(entry[ENTRY_ATTR] & ATTR_SUBDIR ? "<DIR> " : "      ");
}

#pragma aux B_FILS "B$FILS"

/* B$FILS: FILES [pattern]. */
void B_FILS(SD *pattern)
{
    char path[PATH_MAX + 4], directory[66];
    unsigned char entry[ENTRY_BYTES];
    unsigned length = pattern->len, drive = 0, letter;
    long result;

    if (length >= PATH_MAX)
        qb_error(BE_BADNAME);
    copy_bytes(path, pattern->ptr, length);
    str_tmp_free(pattern);
    path[length] = 0;
    if (length >= 2 && path[1] == ':')
        drive = (path[0] & 0xDF) - 'A' + 1;
    if (length == 0 || path[length - 1] == '\\' || path[length - 1] == ':') {
        copy_bytes(path + length, "*.*", 4);
    }
    letter = drive ? 'A' + drive - 1 : 'A' + llrm_os_current_drive();
    if (llrm_os_current_directory(drive, directory) < 0)
        qb_error(BE_NOTFOUND);
    cn_putc(letter);
    put_text(":\\");
    put_text(directory);
    cn_crlf();
    result = llrm_os_find_first((os_path)path, ATTR_SUBDIR, entry);
    if (result < 0)
        qb_error(BE_NOFILE);
    do {
        put_entry(entry);
        if (cn_pos() + 1 + ENTRY_WIDTH >= cn_width())
            cn_crlf();
    } while (llrm_os_find_next(entry) >= 0);
    cn_crlf();
    put_number(llrm_os_free_bytes(drive));
    put_text("Bytes free");
    cn_crlf();
}
