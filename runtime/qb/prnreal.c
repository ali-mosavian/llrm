/* PRINT of a SINGLE or DOUBLE (QB rt/prnval.asm B$P<terminator>R4 and R8),
   apart from prnval.c so that the digit conversion is linked only by a program
   that prints one. */
#include "fout.h"
#include "prnval.h"

static void real(double v, int is_double, enum Terminator end)
{
    char text[FOUT_MAX];

    if (using_ops) {
        using_ops->real(v, is_double);
        using_item_end(end);
        return;
    }
    print_numeral(text, fout_real(v, is_double, text), end);
}

void B_PCR4(float v) { real(v, 0, COMMA); }
void B_PSR4(float v) { real(v, 0, SEMI); }
void B_PER4(float v) { real(v, 0, EOL); }
void B_PCR8(double v) { real(v, 1, COMMA); }
void B_PSR8(double v) { real(v, 1, SEMI); }
void B_PER8(double v) { real(v, 1, EOL); }
#pragma aux B_PCR4 "B$PCR4"
#pragma aux B_PSR4 "B$PSR4"
#pragma aux B_PER4 "B$PER4"
#pragma aux B_PCR8 "B$PCR8"
#pragma aux B_PSR8 "B$PSR8"
#pragma aux B_PER8 "B$PER8"
