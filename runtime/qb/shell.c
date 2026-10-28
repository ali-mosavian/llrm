/* SHELL (QB rt/osstmt.asm B$SSHL, rt/osinit.asm B$BldExecCmdLine): the command interpreter named by COMSPEC
   runs `/C command`, or alone for no command, and the program goes on when it ends. */
#include "llrm_os.h"
#include "nhstutil.h"
#include "qb.h"
#include "shell.h"

enum { TAIL_MAX = 123, ENV_MAX = 128, OUT_OF_MEMORY = 8 };

/* The COMSPEC value in `shell`, or the interpreter QB falls back on. */
static void interpreter(char *shell)
{
    static const char default_shell[] = "COMMAND.COM";
    char line[ENV_MAX];
    long length;
    int index, at;

    for (index = 0; (length = llrm_os_environment(index, line, ENV_MAX - 1)) >= 0; index++) {
        if (length > 8 && line[0] == 'C' && line[1] == 'O' && line[2] == 'M' && line[3] == 'S' &&
            line[4] == 'P' && line[5] == 'E' && line[6] == 'C' && line[7] == '=') {
            for (at = 8; at < length; at++)
                shell[at - 8] = line[at];
            shell[length - 8] = 0;
            return;
        }
    }
    for (at = 0; (shell[at] = default_shell[at]) != 0; at++)
        ;
}

#pragma aux B_SSHL "B$SSHL"

/* B$SSHL: SHELL [command]. Too long a command is Illegal function call; the child failing to load for want of
   memory is Out of memory, any other failure Illegal function call. */
void B_SSHL(SD *command)
{
    char shell[ENV_MAX], tail[TAIL_MAX + 3];
    unsigned length = command->len, at;
    long result;

    if (length > TAIL_MAX - 3) {
        str_tmp_free(command);
        qb_error(BE_ILLFUN);
    }
    tail[1] = 0;
    if (length) {
        tail[1] = '/';
        tail[2] = 'C';
        tail[3] = ' ';
        copy_bytes(tail + 4, command->ptr, length);
        length += 3;
    }
    tail[0] = (char)length;
    tail[length + 1] = '\r';
    str_tmp_free(command);
    interpreter(shell);
    qb_shell_release();
    result = llrm_os_execute((os_path)shell, (os_path)tail);
    qb_shell_reclaim();
    if (result < 0)
        qb_error(-result == OUT_OF_MEMORY ? BE_MEMORY : BE_ILLFUN);
}
