/* LPRINT: PRINT to the printer (QB rt/iolpt.asm), DOS's standard printer
   handle written like a file's. */
#include "device.h"
#include "file.h"

enum { PRINTER_HANDLE = 4 };

static Fdb printer;

/* B$LPRT: the start of an LPRINT statement. */
void B_LPRT(void)
{
    if (!dev_printer_ready())
        qb_error(BE_UNAVAILABLE);
    printer.handle = PRINTER_HANDLE;
    printer.mode = MD_OUTPUT;
    file_print_to(&printer);
}
#pragma aux B_LPRT "B$LPRT"
