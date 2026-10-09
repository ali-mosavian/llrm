/* PRINT of one value (QB rt/prnval.asm B$P<terminator><type>).  The terminator
   is C for a comma, S for a semicolon and E for the end of the statement; the
   type is I2, I4 or SD. */
#include "console.h"
#include "fout.h"
#include "input.h"
#include "using.h"
#include "nhstutil.h"

enum Terminator { COMMA, SEMI, EOL };
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

static void terminate(enum Terminator end)
{
    if (end == EOL) {
        cn_crlf();
    } else if (end == COMMA) {
        byte pad = ZONE - cn_pos() % ZONE;

        if (room(pad + ZONE))
            while (pad--)
                cn_putc(' ');
    }
}

/* A number's text, its trailing space, and the terminator. */
static void numeral(char *text, unsigned length, enum Terminator end)
{
    text[length++] = ' ';
    room(length);
    cn_write(text, length);
    terminate(end);
}

/* The end of an item of a PRINT USING: a separator adds nothing, and the end of
   the statement finishes the format. */
static int using_item_end(enum Terminator end)
{
    if (end == EOL)
        using_end(1);
    return 1;
}

static void number(long v, enum Terminator end)
{
    char text[FOUT_MAX];

    if (using_active()) {
        using_integer(v);
        using_item_end(end);
        return;
    }

    numeral(text, fout_i4(v, text), end);
}

static void real(double v, int is_double, enum Terminator end)
{
    char text[FOUT_MAX];

    if (using_active()) {
        using_real(v, is_double);
        using_item_end(end);
        return;
    }

    numeral(text, fout_real(v, is_double, text), end);
}

static void string(SD *sd, enum Terminator end)
{
    if (using_active()) {
        using_string(sd);
        using_item_end(end);
        return;
    }
    room(sd->len);
    cn_write(sd->ptr, sd->len);
    str_tmp_free(sd);
    terminate(end);
}

/* B$PEOS: the end of a PRINT that ended with a separator, and of an INPUT.  The
   console is written as each item is, so there is nothing to flush. */
void B_PEOS(void)
{
    input_end();
    if (using_active())
        using_end(0);
}

/* B$USNG: PRINT USING, with the format. */
void B_USNG(SD *format)
{
    using_begin(format);
    str_tmp_free(format);
}

void B_PCI2(int v) { number(v, COMMA); }
void B_PSI2(int v) { number(v, SEMI); }
void B_PEI2(int v) { number(v, EOL); }
void B_PCI4(long v) { number(v, COMMA); }
void B_PSI4(long v) { number(v, SEMI); }
void B_PEI4(long v) { number(v, EOL); }
void B_PCR4(float v) { real(v, 0, COMMA); }
void B_PSR4(float v) { real(v, 0, SEMI); }
void B_PER4(float v) { real(v, 0, EOL); }
void B_PCR8(double v) { real(v, 1, COMMA); }
void B_PSR8(double v) { real(v, 1, SEMI); }
void B_PER8(double v) { real(v, 1, EOL); }
void B_PCSD(SD *sd) { string(sd, COMMA); }
void B_PSSD(SD *sd) { string(sd, SEMI); }
void B_PESD(SD *sd) { string(sd, EOL); }
#pragma aux B_PEOS "B$PEOS"
#pragma aux B_USNG "B$USNG"
#pragma aux B_PCI2 "B$PCI2"
#pragma aux B_PSI2 "B$PSI2"
#pragma aux B_PEI2 "B$PEI2"
#pragma aux B_PCI4 "B$PCI4"
#pragma aux B_PSI4 "B$PSI4"
#pragma aux B_PEI4 "B$PEI4"
#pragma aux B_PCR4 "B$PCR4"
#pragma aux B_PSR4 "B$PSR4"
#pragma aux B_PER4 "B$PER4"
#pragma aux B_PCR8 "B$PCR8"
#pragma aux B_PSR8 "B$PSR8"
#pragma aux B_PER8 "B$PER8"
#pragma aux B_PCSD "B$PCSD"
#pragma aux B_PSSD "B$PSSD"
#pragma aux B_PESD "B$PESD"
