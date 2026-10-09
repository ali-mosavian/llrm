/* The error model (QB rt/erproc.asm, erhandlr.asm, inc/messages.inc): one
   number per error, a dispatch to the components that must reset, then the ON
   ERROR handler or the fatal message. */
#include "rtinit.h"
#include "console.h"
#include "error.h"
#include "llrm_os.h"
#include "module.h"

unsigned b_errnum;
unsigned b_inonerr;

/* The text of each error (inc/messages.inc). */
static const struct Message {
    unsigned number;
    const char *text;
} messages[] = {
    {BE_SYNTAX, "Syntax error"},
    {BE_RETURN, "RETURN without GOSUB"},
    {BE_NODATA, "Out of DATA"},
    {BE_ILLFUN, "Illegal function call"},
    {BE_OVERFLOW, "Overflow"},
    {BE_MEMORY, "Out of memory"},
    {BE_SUBSCRIP, "Subscript out of range"},
    {BE_REDIM, "Duplicate definition"},
    {BE_DIVIDE0, "Division by zero"},
    {BE_TYPE, "Type mismatch"},
    {BE_STRINGSP, "Out of string space"},
    {BE_STRINGFO, "String formula too complex"},
    {BE_NORESUME, "No RESUME"},
    {BE_RESUME, "RESUME without error"},
    {24, "Device timeout"},
    {25, "Device fault"},
    {27, "Out of paper"},
    {50, "FIELD overflow"},
    {51, "Internal error"},
    {BE_FILENUM, "Bad file name or number"},
    {BE_NOFILE, "File not found"},
    {BE_FILEMODE, "Bad file mode"},
    {BE_FILEOPEN, "File already open"},
    {56, "FIELD statement active"},
    {BE_DEVICEIO, "Device I/O error"},
    {BE_EXISTS, "File already exists"},
    {59, "Bad record length"},
    {BE_DISKFULL, "Disk full"},
    {BE_PASTEND, "Input past end of file"},
    {BE_BADREC, "Bad record number"},
    {BE_BADNAME, "Bad file name"},
    {BE_TOOMANY, "Too many files"},
    {68, "Device unavailable"},
    {69, "Communication-buffer overflow"},
    {BE_HANDSOFF, "Permission denied"},
    {71, "Disk not ready"},
    {72, "Disk-media error"},
    {73, "Advanced feature unavailable"},
    {74, "Rename across disks"},
    {75, "Path/File access error"},
    {BE_NOTFOUND, "Path not found"},
    {FE_CORRUPT, "String space corrupt"},
    {FE_NOSTACK, "Out of stack space"}
};

const char *qb_error_text(unsigned n)
{
    unsigned at;

    for (at = 0; at < sizeof messages / sizeof messages[0]; at++)
        if (messages[at].number == n)
            return messages[at].text;
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
    unsigned length;

    if (cn_pos())
        cn_crlf();
    write_text(qb_error_text(n));
    write_text(" in line No line number in module ");
    module_name(name);
    for (length = MODULE_NAME_LENGTH; length > 1 && name[length - 1] == ' ';)
        length--;
    cn_write(name, length);
    write_text(" at address ");
    write_hex(module_code_segment());
    write_text(":0000");
    cn_crlf();
    llrm_os_exit(255);
}

void qb_error(unsigned n)
{
    b_errnum = n;
    qb_dispatch(V_ERR);
    fatal(n);
}

void qb_no_resume(void)
{
    qb_error(BE_NORESUME);
}
