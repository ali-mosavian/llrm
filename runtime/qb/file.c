/* Disk files (QB rt/dkopen.asm, dkio.asm, dkutil.asm, dvstmt.asm). */
#include "file.h"
#include "rtinit.h"
#include "llrm_os.h"

enum {
    MAX_CHANNEL = 15,
    DEFAULT_RECORD = 128,
    PATH_MAX = 128,
    ORIGIN_START = 0,
    ORIGIN_END = 2
};

/* QB's number for what DOS refused with (messages.inc). */
static word dos_error(long code)
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

static Fdb *fdb_of(int channel)
{
    return lh_file(channel);
}

static Fdb *open_fdb(int channel)
{
    Fdb *fdb = fdb_of(channel);

    if (!fdb)
        qb_error(BE_FILENUM);
    return fdb;
}

/* The name as DOS wants it: the string and a NUL. */
static void path_of(const SD *name, char *path)
{
    if (name->len == 0 || name->len >= PATH_MAX)
        qb_error(BE_BADNAME);
    copy_bytes(path, name->ptr, name->len);
    path[name->len] = 0;
}

/* DOS's handle for `path` opened as `mode` asks: INPUT must exist, OUTPUT
   starts empty, the others are made if they are missing. */
static short dos_open(const char *path, enum FileMode mode)
{
    const char __far *name = (const char __far *)path;
    long handle;

    if (mode == MD_OUTPUT)
        return llrm_os_create(name);
    handle = llrm_os_open(name, mode == MD_INPUT ? 0 : 2);
    if (handle < 0 && mode != MD_INPUT && -handle == LLRM_OS_NOT_FOUND)
        handle = llrm_os_create(name);
    if (handle < 0)
        qb_error(dos_error(handle));
    return handle;
}

/* B$OPEN: OPEN "name" FOR mode AS #channel LEN = record_length. */
void B_OPEN(SD *name, int channel, int record_length, unsigned mode)
{
    char path[PATH_MAX];
    Fdb *fdb;
    short handle;

    mode &= 0xFF;
    if (channel < 1 || channel > MAX_CHANNEL)
        qb_error(BE_FILENUM);
    if (fdb_of(channel))
        qb_error(BE_FILEOPEN);
    path_of(name, path);
    if (record_length == -1)
        record_length = DEFAULT_RECORD;
    if (record_length <= 0)
        qb_error(BE_ILLFUN);
    handle = dos_open(path, mode);
    fdb = lh_alloc(sizeof(Fdb), LH_FILE, NULL, channel);
    if (!fdb) {
        llrm_os_close(handle);
        qb_error(BE_MEMORY);
    }
    fdb->handle = handle;
    fdb->mode = mode;
    fdb->record_length = record_length;
    if (mode == MD_APPEND)
        llrm_os_seek(handle, 0, ORIGIN_END);
}

static void close_fdb(Fdb *fdb)
{
    llrm_os_close(fdb->handle);
    lh_free(fdb);
}

/* B$CLOS: CLOSE #a, #b ... closes those channels, and a bare CLOSE every file.
   */
void file_close(const int *channels, word count)
{
    Fdb *fdb;

    if (count == 0) {
        while ((fdb = lh_file(0)))
            close_fdb(fdb);
        return;
    }
    while (count--) {
        fdb = fdb_of(*channels++);
        if (fdb)
            close_fdb(fdb);
    }
}

/* B$FLOF: LOF(#channel), the file's size. */
long B_FLOF(int channel)
{
    Fdb *fdb = open_fdb(channel);
    long here = llrm_os_seek(fdb->handle, 0, 1);
    long size = llrm_os_seek(fdb->handle, 0, ORIGIN_END);

    llrm_os_seek(fdb->handle, here, ORIGIN_START);
    return size;
}

/* A record variable of `length` bytes, or the data of a string descriptor when
   the length is 0. */
static void record_of(
    char __far *record,
    int length,
    unsigned char __far **data,
    word *bytes
)
{
    if (length) {
        *data = (unsigned char __far *)record;
        *bytes = length;
    } else {
        SD *sd = (SD *)(word)(unsigned long)record;

        *data = (unsigned char __far *)sd->ptr;
        *bytes = sd->len;
    }
}

void B_GET3(int channel, char __far *record, int length)
{
    Fdb *fdb = open_fdb(channel);
    unsigned char __far *data;
    word bytes;
    long got;

    record_of(record, length, &data, &bytes);
    got = llrm_os_read(fdb->handle, data, bytes);
    if (got < 0)
        qb_error(dos_error(got));
}

void B_PUT3(int channel, char __far *record, int length)
{
    Fdb *fdb = open_fdb(channel);
    unsigned char __far *data;
    word bytes;
    long put;

    record_of(record, length, &data, &bytes);
    put = llrm_os_write_file(fdb->handle, data, bytes);
    if (put < 0)
        qb_error(dos_error(put));
    if ((word)put != bytes)
        qb_error(BE_DISKFULL);
}

/* The end slot closes every file, so END flushes what the program left open
   (rtterm.c). */
static void close_all(void)
{
    file_close(NULL, 0);
}

static Comp comp = { 0, C_DK, { 0, 0, 0, 0, close_all } };

#define XI_FN file_xinit
#include "xi.h"
void file_xinit(void)
{
    qb_comp_add(&comp);
}
#pragma aux B_OPEN "B$OPEN"
#pragma aux B_GET3 "B$GET3"
#pragma aux B_PUT3 "B$PUT3"
#pragma aux B_FLOF "B$FLOF"
#pragma aux file_close "FILE_CLOSE"
