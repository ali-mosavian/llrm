// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* The OS layer's file-name calls: make a directory and a file in it, rename the file, delete it, change into the
   directory and out, remove it; a missing file or directory and a directory still holding a file are the
   interface's negative codes, and every success is 0 (DOS leaves AX undefined there). */
#include "llrm_os.h"
extern void report(long value);
int main(void)
{
    report(llrm_os_mkdir("OSNDIR"));
    report(llrm_os_close(llrm_os_create("OSNDIR\\A.TMP")));
    report(llrm_os_rename("OSNDIR\\A.TMP", "OSNDIR\\B.TMP"));
    report(llrm_os_open("OSNDIR\\A.TMP", 0) == -LLRM_OS_NOT_FOUND);
    report(llrm_os_rmdir("OSNDIR") == -LLRM_OS_DENIED);
    report(llrm_os_delete("OSNDIR\\B.TMP"));
    report(llrm_os_delete("OSNDIR\\B.TMP") < 0); /* DOSBox answers 5 where DOS answers 2 */
    report(llrm_os_chdir("OSNDIR"));
    report(llrm_os_chdir(".."));
    report(llrm_os_chdir("NOSUCH") == -LLRM_OS_PATH_NOT_FOUND);
    report(llrm_os_rmdir("OSNDIR"));
    return 0;
}
