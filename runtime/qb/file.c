/* Disk files (QB rt/dkopen.asm, dkio.asm, dkutil.asm, dvstmt.asm). */
#include "cndriver.h"
#include "console.h"
#include "file.h"
#include "nhstutil.h"
#include "rtinit.h"
#include "llrm_os.h"

enum { MAX_CHANNEL = 15, DEFAULT_RECORD = 128, PATH_MAX = 128, ORIGIN_START = 0, ORIGIN_END = 2, LINE_MAX = 1023 };

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
void B_OPEN(SD *name, short channel, short record_length, unsigned short mode)
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
    str_tmp_free(name);
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

/* B$KILL: KILL "name". One file: no wildcards. As the QB 4.5 runtime's KILL does, it first looks for the file (a
   missing one is File not found whatever DOS would answer to the delete); a refusal is then Path/File access error,
   any other failure File not found. */
void B_KILL(SD *name)
{
    char path[PATH_MAX];
    long result;

    path_of(name, path);
    str_tmp_free(name);
    result = llrm_os_open((os_path)path, 0);
    if (result < 0)
        qb_error(BE_NOFILE);
    llrm_os_close(result);
    result = llrm_os_delete((os_path)path);
    if (result < 0)
        qb_error(-result == LLRM_OS_DENIED ? BE_ACCESS : BE_NOFILE);
}

/* B$NAME: NAME old AS new. An existing new name is File already exists, as the QB 4.5 runtime's NAME answers; a
   different drive is Rename across disks. */
void B_NAME(SD *old, SD *new_name)
{
    char from[PATH_MAX], to[PATH_MAX];
    long result;

    path_of(old, from);
    path_of(new_name, to);
    str_tmp_free(old);
    str_tmp_free(new_name);
    result = llrm_os_rename((os_path)from, (os_path)to);
    if (result >= 0)
        return;
    switch (-result) {
    case LLRM_OS_NOT_FOUND:
        qb_error(BE_NOFILE);
    case LLRM_OS_PATH_NOT_FOUND:
        qb_error(BE_NOTFOUND);
    case 17:
        qb_error(BE_RENAMEDISK);
    default:
        qb_error(BE_EXISTS);
    }
}

/* The directory statements: a missing path is Path not found, a refusal Path/File access error. */
enum DirectoryCall { DIR_MAKE, DIR_CHANGE, DIR_REMOVE };

static void directory_call(SD *name, enum DirectoryCall call)
{
    char path[PATH_MAX];
    long result;

    path_of(name, path);
    str_tmp_free(name);
    if (call == DIR_MAKE)
        result = llrm_os_mkdir((os_path)path);
    else if (call == DIR_CHANGE)
        result = llrm_os_chdir((os_path)path);
    else
        result = llrm_os_rmdir((os_path)path);
    if (result < 0)
        qb_error(-result == LLRM_OS_DENIED ? BE_ACCESS : BE_NOTFOUND);
}

void B_MDIR(SD *name) { directory_call(name, DIR_MAKE); }
void B_CDIR(SD *name) { directory_call(name, DIR_CHANGE); }
void B_RDIR(SD *name) { directory_call(name, DIR_REMOVE); }

/* One buffer serves whichever file is being read or written by character: the
   other operations on the file first give back what it holds. */
enum { BUFFER = 128, END_OF_TEXT = 26, FILE_WIDTH = 255 };

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
        fdb = fdb_of((short)*channels++);
        if (fdb)
            close_fdb(fdb);
    }
}

/* B$FLOF: LOF(#channel), the file's size. */
long B_FLOF(short channel)
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
static void record_of(qb_data_ptr record, int length, os_data_mut *data, unsigned *bytes)
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

void B_GET3(short channel, qb_data_ptr record, short length)
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

void B_PUT3(short channel, qb_data_ptr record, short length)
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
void B_GET4(short channel, long record, qb_data_ptr data, short length)
{
    position_at(open_fdb(channel), record);
    B_GET3(channel, data, length);
}

void B_PUT4(short channel, long record, qb_data_ptr data, short length)
{
    position_at(open_fdb(channel), record);
    B_PUT3(channel, data, length);
}

/* B$SSEK: SEEK #channel, position. */
void B_SSEK(short channel, long position)
{
    position_at(open_fdb(channel), position);
}

/* B$FINP: INPUT$(count [, #channel]) (QB rt/inputf.asm): `count` characters from the keyboard (channel 0), not echoed, waiting
   for each; or from a file, whose end before the last is Input past end (a BINARY file's short read is the shorter string) and which must be open for reading.  An extended key is
   a zero and then its scan code, two characters. */
SD *B_FINP(short count, short channel)
{
    static byte scan_code_waiting;
    Fdb *fdb = NULL;
    char *text;
    SD *made;
    short at;

    if (count < 0)
        qb_error(BE_ILLFUN);
    if (channel != 0 && channel != 255) {
        fdb = open_fdb(channel);
        if (fdb->mode == MD_OUTPUT || fdb->mode == MD_APPEND)
            qb_error(BE_PASTEND);
    }
    made = str_tmp(count, &text);
    for (at = 0; at < count; at++) {
        if (fdb) {
            int c = get_byte(fdb);

            if (c < 0) {
                /* a binary file's short read is the characters there were (QB rt/inputf.asm) */
                if (fdb->mode == MD_BINARY)
                    return str_tmp_copy(made, 0, at);
                qb_error(BE_PASTEND);
            }
            text[at] = (char)c;
        } else if (scan_code_waiting) {
            scan_code_waiting = 0;
            text[at] = (char)llrm_os_console_read_key();
        } else {
            byte code = cn_wait_key();

            if (code == 26)
                qb_end();
            scan_code_waiting = code == 0;
            text[at] = (char)code;
        }
    }
    return made;
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
int B_FEOF(short channel)
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

static void file_init(void)
{
}

static void file_erase(void)
{
}

static void file_sync(void)
{
}

static void file_cursor(int visible)
{
    (void)visible;
}

static byte file_pos(void)
{
    return out_fdb->column;
}

static byte file_line(void)
{
    return 0;
}

static byte file_width(void)
{
    return FILE_WIDTH;
}

static void file_color(int f, int b)
{
    (void)f;
    (void)b;
}

static void file_locate(int r, int c, int k)
{
    (void)r;
    (void)c;
    (void)k;
}

static void file_view(int t, int b)
{
    (void)t;
    (void)b;
}

static void file_size(int c, int r)
{
    (void)c;
    (void)r;
}

static const Driver file_driver = {file_init,   file_write, file_newline, file_erase, file_sync,
                                   file_cursor, file_pos,   file_line,    file_width, file_color,
                                   file_locate, file_sync,  file_view,    file_size};

/* LPRINT: the statement's output goes to this file, a printer's. */
void file_print_to(Fdb *fdb)
{
    out_fdb = fdb;
    cn_redirect(&file_driver);
}

/* B$CHOU: PRINT # or WRITE # channel (0: the screen). */
void B_CHOU(short channel)
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

/* The line an INPUT file is read into: a local heap entry made on first use, not room taken at every OPEN (a program that
   reads no lines, or a DGROUP nearly full, pays nothing) and not a kilobyte of every program's DGROUP (QB reads a line into
   string space).  Compaction would move it, so a hook follows it and the pointer INPUT # keeps into it. */
static char *text_line;

static void line_moved(void *data, int delta)
{
    if (qb_input_line >= (const char *)data && qb_input_line <= (const char *)data + LINE_MAX + 1)
        qb_input_line += delta;
    text_line += delta;
}

/* The next line of an INPUT file, without its line end; past the end of the
   file is Input past end. */
static const char *next_line(void *file)
{
    Fdb *fdb = file;
    unsigned length = 0;
    int c;

    if (!text_line) {
        text_line = lh_alloc(LINE_MAX + 1, LH_LINE, NULL, 0);
        if (!text_line)
            qb_error(BE_MEMORY);
    }
    c = get_byte(fdb);

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
void B_DSKI(short channel)
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
    buffer_release();
    file_close(NULL, 0);
}

static Comp comp = {0, C_DK, {0, 0, 0, 0, close_all}};

#define XI_FN file_xinit
#include "xi.h"

void file_xinit(void)
{
    qb_comp_add(&comp);
    lh_on_move(LH_LINE, line_moved);
}

#pragma aux B_OPEN "B$OPEN"
#pragma aux B_KILL "B$KILL"
#pragma aux B_NAME "B$NAME"
#pragma aux B_MDIR "B$MDIR"
#pragma aux B_CDIR "B$CDIR"
#pragma aux B_RDIR "B$RDIR"
#pragma aux B_GET3 "B$GET3"
#pragma aux B_PUT3 "B$PUT3"
#pragma aux B_FLOF "B$FLOF"
#pragma aux file_close "@file_close@4"
#pragma aux B_GET4 "B$GET4"
#pragma aux B_PUT4 "B$PUT4"
#pragma aux B_SSEK "B$SSEK"
#pragma aux B_FINP "B$FINP"
#pragma aux B_FREF "B$FREF"
#pragma aux B_FEOF "B$FEOF"
#pragma aux B_CHOU "B$CHOU"
#pragma aux B_DSKI "B$DSKI"
