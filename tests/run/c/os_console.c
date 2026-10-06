// flags: -O2 --cpu 486 | -O2 --cpu 486 --target x86-code32
/* The OS layer's write puts exactly `count` bytes on standard output (handle 1): no newline of its own. */
#include "llrm_os.h"
static const char text[] = "ab\r\ncd\r\n";
int main(void)
{
    llrm_os_write_file(1, (const unsigned char *)text, 4);
    llrm_os_write_file(1, (const unsigned char *)text + 4, 4);
    return 0;
}
