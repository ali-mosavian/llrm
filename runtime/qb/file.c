/* Disk files (QB rt/dkopen.asm, dkio.asm, dkutil.asm, dvstmt.asm). */
#include "cndriver.h"
#include "file.h"
#include "nhstutil.h"
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
static short open_file(const char *path, enum FileMode mode)
{
    os_path name = (os_path)path;
    long handle;

    if (mode == MD_OUTPUT)
        return llrm_os_create(name);
    handle = llrm_os_open(name, mode == MD_INPUT ? 0 : 2);
    if (handle < 0 && mode != MD_INPUT && -handle == LLRM_OS_NOT_FOUND)
        handle = llrm_os_create(name);
    if (handle < 0)
        qb_error(os_error(handle));
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
    handle = open_file(path, mode);
    fdb = lh_alloc(sizeof(Fdb), LH_FILE, NULL, channel);
    if (!fdb) {
        llrm_os_close(handle);
        qb_error(BE_MEMORY);
    }
    fdb->handle = handle;
    fdb->mode = mode;
    fdb->record_length = record_length;
    fdb->column = 0;
    if (mode == MD_APPEND)
        llrm_os_seek(handle, 0, ORIGIN_END);
}

/* One buffer serves whichever file is being read or written by character: the
   other operations on the file first give back what it holds. */
enum { BUFFER = 128, LINE_MAX = 1023, END_OF_TEXT = 26, FILE_WIDTH = 255 };

static char buffer[BUFFER];
static Fdb *owner;
static unsigned used, filled;
static byte output_buffered;

static void buffer_release(void)
{
    if (!owner)
        return;
    if (output_buffered) {
        if (used)
            llrm_os_write_file(owner->handle, (os_data)buffer, used);
    } else if (used < filled) {
        llrm_os_seek(owner->handle, (long)used - (long)filled, 1);
    }
    owner = NULL;
    used = filled = 0;
}

static void buffer_for(Fdb *fdb, byte for_output)
{
    if (owner != fdb || output_buffered != for_output) {
        buffer_release();
        owner = fdb;
        output_buffered = for_output;
    }
}

static void put_byte(Fdb *fdb, char c)
{
    buffer_for(fdb, 1);
    if (used == BUFFER) {
        long put = llrm_os_write_file(fdb->handle, (os_data)buffer, used);

        if (put != (long)used)
            qb_error(put < 0 ? os_error(put) : BE_DISKFULL);
        used = 0;
    }
    buffer[used++] = c;
}

/* The next byte of a file read by character, or -1 at its end. */
static int get_byte(Fdb *fdb)
{
    buffer_for(fdb, 0);
    if (used == filled) {
        long got = llrm_os_read(fdb->handle, (os_data_mut)buffer, BUFFER);

        if (got < 0)
            qb_error(os_error(got));
        used = 0;
        filled = got;
        if (got == 0)
            return -1;
    }
    return (byte)buffer[used++];
}

static void close_fdb(Fdb *fdb)
{
    buffer_release();
    llrm_os_close(fdb->handle);
    lh_free(fdb);
}

/* B$CLOS: CLOSE #a, #b ... closes those channels, and a bare CLOSE every file.
   */
void file_close(const int *channels, unsigned count)
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
    long here, size;

    buffer_release();
    here = llrm_os_seek(fdb->handle, 0, 1);
    size = llrm_os_seek(fdb->handle, 0, ORIGIN_END);
    llrm_os_seek(fdb->handle, here, ORIGIN_START);
    return size;
}

/* A record variable of `length` bytes, or the data of a string descriptor when
   the length is 0. */
static void record_of(
    qb_data_ptr record,
    int length,
    os_data_mut *data,
    unsigned *bytes
)
{
    if (length) {
        *data = (os_data_mut)record;
        *bytes = length;
    } else {
        SD *sd = QB_NEAR_OF(record);

        *data = (os_data_mut)sd->ptr;
        *bytes = sd->len;
    }
}

void B_GET3(int channel, qb_data_ptr record, int length)
{
    Fdb *fdb = open_fdb(channel);
    os_data_mut data;
    unsigned bytes;
    long got;

    buffer_release();
    record_of(record, length, &data, &bytes);
    got = llrm_os_read(fdb->handle, data, bytes);
    if (got < 0)
        qb_error(os_error(got));
}

void B_PUT3(int channel, qb_data_ptr record, int length)
{
    Fdb *fdb = open_fdb(channel);
    os_data_mut data;
    unsigned bytes;
    long put;

    buffer_release();
    record_of(record, length, &data, &bytes);
    put = llrm_os_write_file(fdb->handle, data, bytes);
    if (put < 0)
        qb_error(os_error(put));
    if ((unsigned)put != bytes)
        qb_error(BE_DISKFULL);
}

/* Where GET, PUT and SEEK with a position go: a RANDOM file's record, else the
   1-based byte. */
static void position_at(Fdb *fdb, long position)
{
    long offset;

    if (position < 1)
        qb_error(BE_BADREC);
    offset = position - 1;
    if (fdb->mode == MD_RANDOM)
        offset *= fdb->record_length;
    buffer_release();
    llrm_os_seek(fdb->handle, offset, ORIGIN_START);
}

/* B$GET4, B$PUT4: GET and PUT with a record number or byte position. */
void B_GET4(int channel, long record, int length, qb_data_ptr data)
{
    position_at(open_fdb(channel), record);
    B_GET3(channel, data, length);
}

void B_PUT4(int channel, long record, int length, qb_data_ptr data)
{
    position_at(open_fdb(channel), record);
    B_PUT3(channel, data, length);
}

/* B$SSEK: SEEK #channel, position. */
void B_SSEK(int channel, long position)
{
    position_at(open_fdb(channel), position);
}

/* B$FREF: FREEFILE, the lowest channel not in use. */
int B_FREF(void)
{
    int channel = 1;

    while (channel <= MAX_CHANNEL && fdb_of(channel))
        channel++;
    if (channel > MAX_CHANNEL)
        qb_error(BE_TOOMANY);
    return channel;
}

/* B$FEOF: EOF(#channel), true (-1) when a read would find no more. */
int B_FEOF(int channel)
{
    Fdb *fdb = open_fdb(channel);
    int next;

    if (fdb->mode == MD_OUTPUT || fdb->mode == MD_APPEND)
        qb_error(BE_FILEMODE);
    if (fdb->mode == MD_INPUT) {
        next = get_byte(fdb);
        if (next >= 0)
            used--;
        return next < 0 || next == END_OF_TEXT ? -1 : 0;
    }
    return B_FLOF(channel) <= llrm_os_seek(fdb->handle, 0, 1) ? -1 : 0;
}

/* PRINT # and WRITE #: the statement's output goes to this file. */
static Fdb *out_fdb;

static void file_write(const char *text, unsigned count)
{
    while (count--) {
        char c = *text++;

        put_byte(out_fdb, c);
        if (c == '\r' || c == '\n')
            out_fdb->column = 0;
        else if (out_fdb->column < FILE_WIDTH)
            out_fdb->column++;
    }
}

static void file_newline(void)
{
    put_byte(out_fdb, '\r');
    put_byte(out_fdb, '\n');
    out_fdb->column = 0;
}

static void file_init(void) {}
static void file_erase(void) {}
static void file_sync(void) {}
static void file_cursor(int visible) { (void)visible; }
static byte file_pos(void) { return out_fdb->column; }
static byte file_line(void) { return 0; }
static byte file_width(void) { return FILE_WIDTH; }
static void file_color(int f, int b) { (void)f; (void)b; }
static void file_locate(int r, int c, int k) { (void)r; (void)c; (void)k; }
static void file_view(int t, int b) { (void)t; (void)b; }
static void file_size(int c, int r) { (void)c; (void)r; }

static const Driver file_driver = {
    file_init, file_write, file_newline, file_erase, file_sync, file_cursor,
    file_pos, file_line, file_width, file_color, file_locate, file_sync,
    file_view, file_size
};

/* B$CHOU: PRINT # or WRITE # channel (0: the screen). */
void B_CHOU(int channel)
{
    Fdb *fdb;

    if (channel == 0) {
        cn_reset_output();
        return;
    }
    fdb = open_fdb(channel);
    if (fdb->mode == MD_INPUT)
        qb_error(BE_FILEMODE);
    out_fdb = fdb;
    cn_redirect(&file_driver);
}

static char text_line[LINE_MAX + 1];

/* The next line of an INPUT file, without its line end; past the end of the
   file is Input past end. */
static const char *next_line(void *file)
{
    Fdb *fdb = file;
    unsigned length = 0;
    int c = get_byte(fdb);

    if (c < 0 || c == END_OF_TEXT)
        qb_error(BE_PASTEND);
    for (; c >= 0 && c != '\n' && c != END_OF_TEXT; c = get_byte(fdb))
        if (length < LINE_MAX)
            text_line[length++] = (char)c;
    if (c == END_OF_TEXT)
        used--;
    if (length && text_line[length - 1] == '\r')
        length--;
    text_line[length] = 0;
    return text_line;
}

/* B$DSKI: INPUT # or LINE INPUT # channel. */
void B_DSKI(int channel)
{
    Fdb *fdb = open_fdb(channel);

    if (fdb->mode != MD_INPUT && fdb->mode != MD_RANDOM && fdb->mode != MD_BINARY)
        qb_error(BE_FILEMODE);
    qb_input_file = fdb;
    qb_input_file_line = next_line;
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
#pragma aux file_close "@file_close@4"
#pragma aux B_GET4 "B$GET4"
#pragma aux B_PUT4 "B$PUT4"
#pragma aux B_SSEK "B$SSEK"
#pragma aux B_FREF "B$FREF"
#pragma aux B_FEOF "B$FEOF"
#pragma aux B_CHOU "B$CHOU"
#pragma aux B_DSKI "B$DSKI"
