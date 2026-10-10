/* COMMAND$ and ENVIRON$ (QB rt/osstmt.asm): the program's command line and
   environment, as the OS layer gives them. */
#include "llrm_os.h"
#include "nhstutil.h"
#include "qb.h"

enum { TEXT_MAX = 255 };

static char text[TEXT_MAX];

/* B$FCMD: COMMAND$, the command line in capitals. */
SD *B_FCMD(void)
{
    char *data;
    SD *result;
    long length = llrm_os_command_line((os_data_mut)text, TEXT_MAX);
    long at;

    result = str_tmp(length, &data);
    for (at = 0; at < length; at++)
        data[at] = text[at] >= 'a' && text[at] <= 'z' ? text[at] - 32 : text[at];
    return result;
}

static SD *made(long length)
{
    char *data;
    SD *result = str_tmp(length < 0 ? 0 : length, &data);

    if (length > 0)
        copy_bytes(data, text, length);
    return result;
}

/* B$FEVI: ENVIRON$(n), the nth variable as NAME=VALUE, or "" past the last. */
SD *B_FEVI(int n)
{
    if (n < 1)
        qb_error(BE_ILLFUN);
    return made(llrm_os_environment(n - 1, (os_data_mut)text, TEXT_MAX));
}

/* B$FEVS: ENVIRON$(name), the value of the variable, or "" without it.  The name is matched as typed. */
SD *B_FEVS(SD *name)
{
    long length, at;
    unsigned i;

    if (name->len == 0)
        qb_error(BE_ILLFUN);
    for (at = 0; (length = llrm_os_environment(at, (os_data_mut)text, TEXT_MAX)) >= 0; at++) {
        for (i = 0; i < name->len && (long)i < length && text[i] == name->ptr[i]; i++)
            ;
        if (i == name->len && (long)i < length && text[i] == '=') {
            long value = length - i - 1;

            str_tmp_free(name);
            copy_bytes(text, text + i + 1, value);
            return made(value);
        }
    }
    str_tmp_free(name);
    return made(0);
}
#pragma aux B_FCMD "B$FCMD"
#pragma aux B_FEVI "B$FEVI"
#pragma aux B_FEVS "B$FEVS"
