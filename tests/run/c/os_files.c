// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* The OS layer's file calls: create, write, close, open, read, close; a missing file and a bad handle are
   the interface's negative codes, and close is 0 on success (DOS leaves AX undefined there). */
#include "llrm_os.h"
extern void report(long value);
static unsigned char buffer[8];
int main(void)
{
    short out = llrm_os_create("OSFILESC.TMP");
    short in;
    int i;
    report(llrm_os_write_file(out, (const unsigned char *)"hello!", 6));
    report(llrm_os_close(out));
    in = llrm_os_open("OSFILESC.TMP", 0);
    report(llrm_os_read(in, buffer, 8));
    report(llrm_os_close(in));
    report(llrm_os_open("NOSUCH.TMP", 0) == -LLRM_OS_NOT_FOUND);
    report(llrm_os_read(99, buffer, 1) < 0);
    report(llrm_os_close(99) == -6);
    for (i = 0; i < 6; i++) report(buffer[i]);
    return 0;
}
