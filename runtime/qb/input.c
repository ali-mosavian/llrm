/* INPUT from the console (QB rt/inptty.asm B$INPP).

   B$INPP prompts, reads a line and checks it against the types of the
   variables, asking again with "Redo from start" while it does not fit.  The
   line then stays here, and each variable's B$RDxx takes its item from it. */
#include "console.h"
#include "error.h"
#include "fin.h"
#include "rtinit.h"
#include "llrm_os.h"
#include "nhstutil.h"

enum {
    LINE_MAX = 255,
    ENTER = 13,
    BACKSPACE = 8,
    END_OF_FILE = 26,
    NO_QUESTION_MARK = 1,
    KEEP_LINE = 2,
    TYPE_I2 = 0x02,
    TYPE_SD = 0x03,
    TYPE_R4 = 0x04,
    TYPE_R8 = 0x08,
    TYPE_I4 = 0x14
};

/* The block B$INPP is passed: one more than the number of variables, the flags,
   then a byte for the type of each. */
typedef struct InputBlock {
    u16 variables_plus_one;
    byte flags;
    byte type[1];
} InputBlock;

static char line[LINE_MAX + 1];

/* Reads and shows a line, with the backspace key to correct it. */
static void read_line(void)
{
    unsigned used = 0;
    byte key;

    for (;;) {
        cn_sync();
        key = llrm_os_console_read_key();
        if (key == END_OF_FILE)
            qb_error(BE_PASTEND);
        if (key == ENTER)
            break;
        if (key == 0) {
            llrm_os_console_read_key();
        } else if (key == BACKSPACE) {
            if (used) {
                used--;
                cn_erase();
            }
        } else if (used < LINE_MAX && key >= ' ') {
            line[used++] = (char)key;
            cn_putc((char)key);
        }
    }
    line[used] = '\0';
}

/* Whether the line holds exactly the values the types call for. */
static int fits_values(const InputBlock QB_FAR *block, unsigned count)
{
    const char *at = line;
    unsigned item;

    for (item = 0; item < count; item++) {
        char end;

        if (block->type[item] == TYPE_SD) {
            const char *start;
            unsigned length;

            end = fin_string(&at, &start, &length);
        } else {
            FinValue value;
            byte type = block->type[item] == TYPE_I2 ? VT_I2
                      : block->type[item] == TYPE_I4 ? VT_I4
                      : block->type[item] == TYPE_R4 ? VT_R4 : VT_R8;

            end = fin_number(&at, type, &value);
        }
        if (end != ',' && end != '\0')
            return 0;
        if ((end == '\0') != (item == count - 1))
            return 0;
    }
    return 1;
}

/* The check, and the error of a number that does not fit its variable. */
static int fits(const InputBlock QB_FAR *block, unsigned count)
{
    int fit;

    fin_trap(1);
    fit = fits_values(block, count);
    fin_trap(0);
    return fit && fin_error == 0;
}

/* B$INPP: the prompt, the line, and the check. */
void B_INPP(SD *prompt, const InputBlock QB_FAR *block)
{
    unsigned count = block->variables_plus_one - 1;
    static const char redo[] = "Redo from start";

    for (;;) {
        cn_write(prompt->ptr, prompt->len);
        if (!(block->flags & NO_QUESTION_MARK))
            cn_write("? ", 2);
        read_line();
        if (!(block->flags & KEEP_LINE))
            cn_crlf();
        if (fits(block, count))
            break;
        cn_crlf();
        if (fin_error) {
            const char *text = qb_error_text(fin_error);
            unsigned length = 0;

            while (text[length])
                length++;
            cn_write(text, length);
            cn_crlf();
        }
        cn_write(redo, sizeof redo - 1);
        cn_crlf();
    }
    str_tmp_free(prompt);
    qb_input_line = line;
}
#pragma aux B_INPP "B$INPP"
