/* PRINT of one value (QB rt/prnval.asm B$P<terminator><type>).  The terminator is C for a comma,
   S for a semicolon and E for the end of the statement; the type is I2, I4 or SD.  The frontend
   passes the value and the entry removes it, so each is a fixed-arity entry here. */
#include "console.h"
#include "fout.h"
#include "nhstutil.h"

enum { COMMA, SEMI, EOL };
enum { ZONE = 14 };

/* B$PRTCHK: make room for `len` more characters on the line, starting a new one when they do not fit.
   False when it did (the caller then has nothing left to align). */
static byte room(word len)
{
    byte pos = cn_pos(), width = cn_width();

    if (len < 256 && pos <= width && width - pos >= len)
        return 1;
    if (pos)
        cn_crlf();
    return 0;
}

static void terminate(byte term)
{
    byte pad, n;

    if (term == EOL) {
        cn_crlf();
    } else if (term == COMMA) {
        pad = ZONE - cn_pos() % ZONE;
        if (room(pad + ZONE))
            for (n = 0; n < pad; n++)
                cn_putc(' ');
    }
}

static void number(long v, byte term)
{
    byte text[FOUT_MAX];
    word len = fout_i4(v, text);

    text[len++] = ' ';
    room(len);
    cn_write(text, len);
    terminate(term);
}

static void string(word sd, byte term)
{
    SD *s = (SD *)sd;

    room(s->len);
    cn_write((const byte *)s->ptr, s->len);
    str_tmp_free(s);
    terminate(term);
}

void QB B_PCI2(int v) { number(v, COMMA); }
void QB B_PSI2(int v) { number(v, SEMI); }
void QB B_PEI2(int v) { number(v, EOL); }
void QB B_PCI4(long v) { number(v, COMMA); }
void QB B_PSI4(long v) { number(v, SEMI); }
void QB B_PEI4(long v) { number(v, EOL); }
void QB B_PCSD(word sd) { string(sd, COMMA); }
void QB B_PSSD(word sd) { string(sd, SEMI); }
void QB B_PESD(word sd) { string(sd, EOL); }
#pragma aux B_PCI2 "B$PCI2"
#pragma aux B_PSI2 "B$PSI2"
#pragma aux B_PEI2 "B$PEI2"
#pragma aux B_PCI4 "B$PCI4"
#pragma aux B_PSI4 "B$PSI4"
#pragma aux B_PEI4 "B$PEI4"
#pragma aux B_PCSD "B$PCSD"
#pragma aux B_PSSD "B$PSSD"
#pragma aux B_PESD "B$PESD"
