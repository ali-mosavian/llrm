// flags: -O2 --cpu 486 | -O2 --cpu 486 --target x86-code32
// stdin: conin.dat
/* The OS layer's console follows redirection: standard input is conin.dat ("Q", "ab", CR LF), standard output a
   file. A key is read without waiting once one is ready, a line is `read` on the standard input handle, and a
   read at the end of the input returns 0. */
#include "llrm_os.h"
extern void report(long value);
static unsigned char buffer[8];
int main(void)
{
    report(llrm_os_console_key_ready());
    report(llrm_os_console_read_key());
    report(llrm_os_read(LLRM_OS_STDIN, buffer, 3));
    report(buffer[0]);
    report(buffer[2]);
    report(llrm_os_console_key_ready());
    report(llrm_os_console_read_key());
    report(llrm_os_read(LLRM_OS_STDIN, buffer, 3));
    llrm_os_write_file(LLRM_OS_STDOUT, (const unsigned char *)"ok\r\n", 4);
    return 0;
}
