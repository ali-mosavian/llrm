/* PRINT of one value (QB rt/prnval.asm B$P<terminator><type>).  The terminator
   is C for a comma, S for a semicolon and E for the end of the statement; the
   type is I2, I4 or SD. */
#include "console.h"
#include "fout.h"
#include "prnval.h"
#include "rtinit.h"
enum { ZONE = 14 };

/* B$PRTCHK: make room for `len` more characters on the line, ending it first
   when they do not fit. False when it did. */
static int room(unsigned len)
{
    byte pos = cn_pos(), width = cn_width();

    if (len < 256 && pos <= width && width - pos >= len)
        return 1;
    if (pos)
        cn_crlf();
    return 0;
}

/* Set by B$WRIT: the statement is WRITE, which quotes strings, leaves numbers
   without blanks and puts a comma after every item. */
static byte writing;

static void terminate(enum Terminator end)
{
    if (end == EOL) {
        cn_crlf();
        writing = 0;
        cn_reset_output();
    } else if (writing) {
        cn_putc(',');
    } else if (end == COMMA) {
        byte pad = ZONE - cn_pos() % ZONE;

        if (room(pad + ZONE))
            while (pad--)
                cn_putc(' ');
    }
}

const UsingOps *using_ops;

void print_numeral(char *text, unsigned length, enum Terminator end)
{
    if (writing) {
        if (*text == ' ') {
            text++;
            length--;
        }
        cn_write(text, length);
    } else {
        text[length++] = ' ';
        room(length);
        cn_write(text, length);
    }
    terminate(end);
}

void using_item_end(enum Terminator end)
{
    if (end == EOL)
        using_ops->end(1);
}

static void number(long v, enum Terminator end)
{
    char text[FOUT_MAX];

    if (using_ops) {
        using_ops->integer(v);
        using_item_end(end);
        return;
    }
    print_numeral(text, fout_i4(v, text), end);
}

static void string(SD *sd, enum Terminator end)
{
    if (using_ops) {
        using_ops->string(sd);
        using_item_end(end);
        return;
    }
    if (writing) {
        cn_putc('"');
        cn_write(sd->ptr, sd->len);
        cn_putc('"');
    } else {
        room(sd->len);
        cn_write(sd->ptr, sd->len);
    }
    str_tmp_free(sd);
    terminate(end);
}

/* B$FTAB: TAB(column) in a PRINT: spaces up to the 1-based column, on the next
   line if the cursor is past it. */
void B_FTAB(int column)
{
    int width = cn_width(), target, pos = cn_pos();

    if (column < 1)
        column = 1;
    target = (column - 1) % width;
    if (target < pos) {
        cn_crlf();
        pos = 0;
    }
    while (pos++ < target)
        cn_putc(' ');
}
#pragma aux B_FTAB "B$FTAB"

/* B$WRIT: the start of a WRITE statement. */
void B_WRIT(void)
{
    writing = 1;
}
#pragma aux B_WRIT "B$WRIT"

/* B$PEOS: the end of a PRINT that ended with a separator, and of an INPUT.  The
   console is written as each item is, so there is nothing to flush. */
void B_PEOS(void)
{
    qb_input_line = 0;
    qb_input_file = 0;
    writing = 0;
    cn_reset_output();
    if (using_ops)
        using_ops->end(0);
}

void B_PCI2(int v) { number(v, COMMA); }
void B_PSI2(int v) { number(v, SEMI); }
void B_PEI2(int v) { number(v, EOL); }
void B_PCI4(long v) { number(v, COMMA); }
void B_PSI4(long v) { number(v, SEMI); }
void B_PEI4(long v) { number(v, EOL); }
void B_PCSD(SD *sd) { string(sd, COMMA); }
void B_PSSD(SD *sd) { string(sd, SEMI); }
void B_PESD(SD *sd) { string(sd, EOL); }
#pragma aux B_PEOS "B$PEOS"
#pragma aux B_PCI2 "B$PCI2"
#pragma aux B_PSI2 "B$PSI2"
#pragma aux B_PEI2 "B$PEI2"
#pragma aux B_PCI4 "B$PCI4"
#pragma aux B_PSI4 "B$PSI4"
#pragma aux B_PEI4 "B$PEI4"
#pragma aux B_PCSD "B$PCSD"
#pragma aux B_PSSD "B$PSSD"
#pragma aux B_PESD "B$PESD"
