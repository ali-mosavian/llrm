/* INKEY$ (QB rt/gwkey.asm B$INKY): the next key typed, without waiting. */
#include "console.h"
#include "error.h"
#include "llrm_os.h"
#include "nhstutil.h"

enum { CTRL_Z = 26 };

int (*cn_key_hook)(void);

/* The empty string when no key is waiting; a key is one character, and an
   extended key (function and cursor keys) is a zero and its scan code.  The end
   of redirected input ends the program. */
SD *B_INKY(void)
{
    char *text;
    SD *key;
    byte code;

    cn_sync();
    for (;;) {
        if (cn_key_hook)
            cn_key_hook();
        if (!llrm_os_console_key_ready())
            return &str_nul;
        /* a key that came in as the hook looked is read once it is no trap's */
        if (!cn_key_hook || !cn_key_hook())
            break;
    }
    code = llrm_os_console_read_key();
    if (code == CTRL_Z)
        qb_end();
    if (code != 0) {
        key = str_tmp(1, &text);
        text[0] = (char)code;
        return key;
    }
    key = str_tmp(2, &text);
    text[0] = 0;
    text[1] = (char)llrm_os_console_read_key();
    return key;
}

#pragma aux B_INKY "B$INKY"
