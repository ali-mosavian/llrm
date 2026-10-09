/* The error model (QB rt/erproc.asm, erhandlr.asm, inc/messages.inc): one
   number per error, a dispatch to the components that must reset, then the ON
   ERROR handler or the fatal message. */
#include "rtinit.h"
#include "console.h"
#include "error.h"
#include "llrm_os.h"
#include "module.h"
#include "nhstutil.h"

unsigned b_errnum;
unsigned b_inonerr;

/* The text of each error (inc/messages.inc), packed: a code byte (the number,
   or 200 + the low byte of a fatal one), the text and a NUL. */
static const char messages[] =
    "\x02" "Syntax error\0"
    "\x03" "RETURN without GOSUB\0"
    "\x04" "Out of DATA\0"
    "\x05" "Illegal function call\0"
    "\x06" "Overflow\0"
    "\x07" "Out of memory\0"
    "\x09" "Subscript out of range\0"
    "\x0a" "Duplicate definition\0"
    "\x0b" "Division by zero\0"
    "\x0d" "Type mismatch\0"
    "\x0e" "Out of string space\0"
    "\x10" "String formula too complex\0"
    "\x13" "No RESUME\0"
    "\x14" "RESUME without error\0"
    "\x18" "Device timeout\0"
    "\x19" "Device fault\0"
    "\x1b" "Out of paper\0"
    "\x32" "FIELD overflow\0"
    "\x33" "Internal error\0"
    "\x34" "Bad file name or number\0"
    "\x35" "File not found\0"
    "\x36" "Bad file mode\0"
    "\x37" "File already open\0"
    "\x38" "FIELD statement active\0"
    "\x39" "Device I/O error\0"
    "\x3a" "File already exists\0"
    "\x3b" "Bad record length\0"
    "\x3d" "Disk full\0"
    "\x3e" "Input past end of file\0"
    "\x3f" "Bad record number\0"
    "\x40" "Bad file name\0"
    "\x43" "Too many files\0"
    "\x44" "Device unavailable\0"
    "\x45" "Communication-buffer overflow\0"
    "\x46" "Permission denied\0"
    "\x47" "Disk not ready\0"
    "\x48" "Disk-media error\0"
    "\x49" "Advanced feature unavailable\0"
    "\x4a" "Rename across disks\0"
    "\x4b" "Path/File access error\0"
    "\x4c" "Path not found\0"
    "\xc8" "String space corrupt\0"
    "\xcf" "Out of stack space\0";

const char *qb_error_text(unsigned n)
{
    const char *at = messages;
    byte want = n < 200 ? (byte)n : (byte)(200 + (n & 0xFF));

    while (at < messages + sizeof messages - 1) {
        if ((byte)*at == want)
            return at + 1;
        while (*++at)
            ;
        at++;
    }
    return "Unprintable error";
}

static void write_text(const char *text)
{
    unsigned length = 0;

    while (text[length])
        length++;
    cn_write(text, length);
}

static void write_hex(unsigned value)
{
    char digits[4];
    unsigned at;

    for (at = 4; at--; value >>= 4)
        digits[at] = "0123456789ABCDEF"[value & 15];
    cn_write(digits, 4);
}

/* The message an error nothing handles ends the program with. */
static void fatal(unsigned n)
{
    char name[MODULE_NAME_LENGTH];

    cn_crlf();
    write_text(qb_error_text(n));
    write_text(" in line No line number in module ");
    module_name(name);
    cn_write(name, MODULE_NAME_LENGTH);
    write_text(" at address ");
    write_hex(module_code_segment());
    write_text(":0000");
    cn_crlf();
    cn_sync();
    cn_waiting(1);
    llrm_os_exit(255);
}

/* The frame the module's code registered its handler from (land.asm), and where
   the handler is. */
unsigned qb_land_sp, qb_land_bp, qb_land_to, qb_err_ip;
void qb_land(void);

/* B$OEGA: the handler's offset in the module's code, or 0 for none.  A handler
   set again is how the compiled RESUME ends the error: ERR is 0 after it. */
void on_error(unsigned target)
{
    md_set_on_error(module_data(), target);
    b_inonerr = 0;
    if (target)
        b_errnum = 0;
}

/* B$SERR: ERROR n; 0 and numbers past 255 are Illegal function call. */
void raise(unsigned n)
{
    if (n == 0 || n > 255)
        n = BE_ILLFUN;
    qb_error(n);
}

void qb_error(unsigned n)
{
    unsigned handler;

    b_errnum = n;
    qb_dispatch(V_ERR);
    handler = md_on_error(module_data());
    if (handler && n < 256 && !b_inonerr) {
        str_all_tmp_free(0);
        b_inonerr = 1;
        qb_land_to = handler;
        qb_land();
    }
    fatal(n);
}

void qb_no_resume(void)
{
    qb_error(BE_NORESUME);
}
#pragma aux qb_land "QB_LAND"
#pragma aux on_error "@on_error@2"
#pragma aux raise "@raise@2"
