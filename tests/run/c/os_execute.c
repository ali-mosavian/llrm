// flags: -O2 -march=i486 -DREAL_MODE | -O2 -march=i486 -m32
/* The OS layer's load-and-execute: the command interpreter runs a command that writes a file, which is there after;
   a program that is not there is an error. */
#include "llrm_os.h"
extern void report(long value);
#ifdef REAL_MODE
/* A real-mode program owns all the memory DOS had; the child needs some back. */
extern unsigned short llrm_os_psp;
#pragma aux llrm_os_psp "LL$PSP"
#endif
static char shell[80], tail[40], contents[16];
static int env_shell(void)
{
    char line[96];
    int index, at, length;
    for (index = 0; (length = llrm_os_environment(index, line, 95)) >= 0; index++)
        if (length > 8 && line[0] == 'C' && line[1] == 'O' && line[2] == 'M' && line[3] == 'S' && line[4] == 'P' && line[5] == 'E' && line[6] == 'C' && line[7] == '=') {
            for (at = 8; at < length; at++)
                shell[at - 8] = line[at];
            shell[length - 8] = 0;
            return 1;
        }
    return 0;
}
int main(void)
{
    static const char command[] = " /C ECHO x>OSEXEC.TMP\r";
    short handle;
    int at;
    report(env_shell());
    for (at = 0; command[at]; at++)
        tail[at + 1] = command[at];
    tail[0] = at - 1;
#ifdef REAL_MODE
    llrm_os_block_resize(llrm_os_psp, 0x2000);
#endif
    report(llrm_os_execute(shell, tail));
#ifdef REAL_MODE
    llrm_os_block_resize(llrm_os_psp, 0xFFFF);
#endif
    handle = llrm_os_open("OSEXEC.TMP", 0);
    report(handle >= 0);
    report(llrm_os_read(handle, (unsigned char *)contents, 1));
    report(contents[0] == 'x');
    llrm_os_close(handle);
    tail[0] = 0;
    tail[1] = '\r';
    report(llrm_os_execute("NOSUCH.EXE", tail) < 0);
    return 0;
}
