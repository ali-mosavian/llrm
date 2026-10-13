/* DATE$ and TIME$ (QB rt/osdate.asm): the date and the time of DOS as text,
   and set from text. */
#include "device.h"
#include "error.h"
#include "nhstutil.h"
#include "qb.h"

enum { DOS_GET_DATE = 0x2A00, DOS_SET_DATE = 0x2B00 };

enum { DOS_GET_TIME = 0x2C00, DOS_SET_TIME = 0x2D00 };

static void put2(char *to, unsigned value)
{
    to[0] = (char)('0' + value / 10 % 10);
    to[1] = (char)('0' + value % 10);
}

/* B$FDAT: DATE$, mm-dd-yyyy. */
SD *B_FDAT(void)
{
    Regs r;
    char *text;
    SD *date = str_tmp(10, &text);

    r.rax = DOS_GET_DATE;
    dev_int21(&r);
    put2(text, r.rdx >> 8);
    text[2] = '-';
    put2(text + 3, r.rdx & 0xFF);
    text[5] = '-';
    put2(text + 6, r.rcx / 100);
    put2(text + 8, r.rcx % 100);
    return date;
}

/* B$FTIM: TIME$, hh:mm:ss. */
SD *B_FTIM(void)
{
    Regs r;
    char *text;
    SD *time = str_tmp(8, &text);

    r.rax = DOS_GET_TIME;
    dev_int21(&r);
    put2(text, r.rcx >> 8);
    text[2] = ':';
    put2(text + 3, r.rcx & 0xFF);
    text[5] = ':';
    put2(text + 6, r.rdx >> 8);
    return time;
}

typedef struct Scan {
    const char *at, *end;
} Scan;

/* One or two digits as a number; anything else is an illegal function call. */
static unsigned digits(Scan *s, int *count)
{
    unsigned value = 0;

    *count = 0;
    while (*count < 2 && s->at < s->end && *s->at >= '0' && *s->at <= '9') {
        value = value * 10 + (unsigned)(*s->at++ - '0');
        (*count)++;
    }
    if (!*count)
        qb_error(BE_ILLFUN);
    return value;
}

/* The separator after a number: one of `separators`; at the end of the text
   there is none (-1). */
static int separator(Scan *s, const char *separators)
{
    char c;

    if (s->at >= s->end)
        return -1;
    c = *s->at++;
    while (*separators)
        if (*separators++ == c)
            return 1;
    qb_error(BE_ILLFUN);
    return 0;
}

/* B$SDAT: DATE$ = "mm-dd-yy" or "mm/dd/yyyy". */
void B_SDAT(SD *text)
{
    Scan s;
    Regs r;
    int count;
    unsigned month, day, year;

    s.at = text->ptr;
    s.end = text->ptr + text->len;
    if (!text->len)
        qb_error(BE_ILLFUN);
    month = digits(&s, &count);
    if (separator(&s, "-/") < 0)
        qb_error(BE_ILLFUN);
    day = digits(&s, &count);
    if (separator(&s, "-/") < 0)
        qb_error(BE_ILLFUN);
    year = digits(&s, &count);
    if (s.at < s.end) {
        /* four digits: the first two are the century */
        unsigned low = digits(&s, &count);

        if (count < 2)
            qb_error(BE_ILLFUN);
        year = year * 100 + low;
    } else {
        year += 1900;
    }
    r.rax = DOS_SET_DATE;
    r.rcx = year;
    r.rdx = month << 8 | day;
    dev_int21(&r);
    if ((r.rax & 0xFF) != 0)
        qb_error(BE_ILLFUN);
    str_tmp_free(text);
}

/* B$STIM: TIME$ = "hh", "hh:mm" or "hh:mm:ss". */
void B_STIM(SD *text)
{
    Scan s;
    Regs r;
    int count;
    unsigned hours, minutes = 0, seconds = 0;

    s.at = text->ptr;
    s.end = text->ptr + text->len;
    if (!text->len)
        qb_error(BE_ILLFUN);
    hours = digits(&s, &count);
    if (separator(&s, ":.") > 0) {
        minutes = digits(&s, &count);
        if (separator(&s, ":.") > 0)
            seconds = digits(&s, &count);
    }
    r.rax = DOS_SET_TIME;
    r.rcx = hours << 8 | minutes;
    r.rdx = seconds << 8;
    dev_int21(&r);
    if ((r.rax & 0xFF) != 0)
        qb_error(BE_ILLFUN);
    str_tmp_free(text);
}

#pragma aux B_FDAT "B$FDAT"
#pragma aux B_FTIM "B$FTIM"
#pragma aux B_SDAT "B$SDAT"
#pragma aux B_STIM "B$STIM"
