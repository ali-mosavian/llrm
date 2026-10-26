// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* The OS layer's directory search: a pattern finds the files made, in some order, with their sizes and names; no
   match is an error; the current directory and the free space are there. */
#include "llrm_os.h"
extern void report(long value);
static unsigned char entry[43];
static char directory[65];
static void make(const char *name, const char *text, int length)
{
    short handle = llrm_os_create(name);
    llrm_os_write_file(handle, (const unsigned char *)text, length);
    llrm_os_close(handle);
}
int main(void)
{
    int found = 0, sizes = 0, seen = 0;
    make("OSFIND1.TMP", "abc", 3);
    make("OSFIND2.TMP", "hello!", 6);
    if (llrm_os_find_first("OSFIND?.TMP", 0, entry) == 0)
        do {
            found++;
            sizes += *(unsigned long *)(entry + 26);
            seen |= 1 << (entry[36] - '1');
        } while (llrm_os_find_next(entry) == 0);
    report(seen);
    report(found);
    report(sizes);
    report(llrm_os_find_first("OSNOSUCH.*", 0, entry) < 0);
    report(llrm_os_current_directory(0, directory));
    report(llrm_os_current_drive() >= 2);
    report(llrm_os_free_bytes(0) > 0);
    report(llrm_os_free_bytes(99) < 0);
    return 0;
}
