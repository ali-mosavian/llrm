/* BLOAD and BSAVE (QB rt/bload.asm): a file of a 0xFD byte, the segment, offset and length of the memory it holds (words), then
   that memory.  The file is the runtime's own, not a channel; a name with no extension takes ".BAS" (B$OPENIT on file 0).  An
   object of its own: a program that does not BLOAD links neither this nor DEF SEG's. */
#include "error.h"
#include "llrm_os.h"
#include "nhstutil.h"
#include "qb.h"

enum { PATH_MAX = 128, BSAVE_TYPE = 0xFD, BSAVE_HEADER = 7 };

#pragma aux b_seg "b$seg"
#pragma aux B_BLOD "B$BLOD"
#pragma aux B_BSAV "B$BSAV"

extern u16 b_seg;

/* QB's number for what DOS refused with (messages.inc). */
static unsigned os_error(long code)
{
    switch (-code) {
    case LLRM_OS_NOT_FOUND:
        return BE_NOFILE;
    case LLRM_OS_PATH_NOT_FOUND:
        return BE_NOTFOUND;
    case LLRM_OS_DENIED:
        return BE_HANDSOFF;
    default:
        return BE_DEVICEIO;
    }
}

static void bsave_path(const SD *name, char *path)
{
    unsigned at, dot = 0;

    if (name->len == 0 || name->len >= PATH_MAX)
        qb_error(BE_BADNAME);
    copy_bytes(path, name->ptr, name->len);
    path[name->len] = 0;
    for (at = 0; path[at]; at++) {
        if (path[at] == '\\' || path[at] == '/' || path[at] == ':')
            dot = 0;
        else if (path[at] == '.')
            dot = 1;
    }
    if (!dot) {
        if (at + 4 >= PATH_MAX)
            qb_error(BE_BADNAME);
        copy_bytes(path + at, ".BAS", 5);
    }
}

/* B$BLOD: BLOAD name[, offset]; `supplied` is 0 when the file's own address is to be used. */
void B_BLOD(SD *name, unsigned offset, short supplied)
{
    char path[PATH_MAX];
    unsigned char header[BSAVE_HEADER];
    long handle;
    long got;
    u16 segment, length;
    unsigned long at;

    bsave_path(name, path);
    str_tmp_free(name);
    handle = llrm_os_open((os_path)path, 0);
    if (handle < 0)
        qb_error(os_error(handle));
    got = llrm_os_read(handle, header, BSAVE_HEADER);
    if (got != BSAVE_HEADER || header[0] != BSAVE_TYPE) {
        llrm_os_close(handle);
        qb_error(BE_FILEMODE);
    }
    segment = header[1] | header[2] << 8;
    at = header[3] | header[4] << 8;
    length = header[5] | header[6] << 8;
    if (supplied) {
        segment = b_seg;
        at = offset;
    }
    got = length ? llrm_os_read(handle, (os_data_mut)QB_REAL_POINTER(segment, at), length) : 0;
    llrm_os_close(handle);
    if (got != (long)length)
        qb_error(BE_FILEMODE);
}

/* B$BSAV: BSAVE name, offset, length, from the DEF SEG segment. */
void B_BSAV(SD *name, unsigned offset, unsigned short length)
{
    char path[PATH_MAX];
    unsigned char header[BSAVE_HEADER];
    long handle;
    long put = BSAVE_HEADER;

    bsave_path(name, path);
    str_tmp_free(name);
    header[0] = BSAVE_TYPE;
    header[1] = (unsigned char)b_seg;
    header[2] = (unsigned char)(b_seg >> 8);
    header[3] = (unsigned char)offset;
    header[4] = (unsigned char)(offset >> 8);  /* the low word: the image keeps QB's 16-bit offset */
    header[5] = (unsigned char)length;
    header[6] = (unsigned char)(length >> 8);
    handle = llrm_os_create((os_path)path);
    if (handle < 0)
        qb_error(os_error(handle));
    if (llrm_os_write_file(handle, header, BSAVE_HEADER) != BSAVE_HEADER)
        put = -1;
    if (put > 0 && length)
        put = llrm_os_write_file(handle, (os_data)QB_REAL_POINTER(b_seg, offset), length) == (long)length ? 0 : -1;
    llrm_os_close(handle);
    if (put < 0)
        qb_error(BE_DISKFULL);
}
