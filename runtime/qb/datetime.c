/* DATE$ and TIME$ (QB rt/osdate.asm): the date and the time of the clock as text,
   and set from text. */
#include "dev.h"
#include "error.h"
#include "nhstutil.h"
#include "qb.h"

static void put2(char *to, unsigned value)
{
    to[0] = (char)('0' + value / 10 % 10);
    to[1] = (char)('0' + value % 10);
}

#pragma aux B_DTSR "B$DTSR"
#pragma aux B_FDAT "B$FDAT"
#pragma aux B_FTIM "B$FTIM"
#pragma aux B_SDAT "B$SDAT"
#pragma aux B_STIM "B$STIM"

/* B$FDAT: DATE$, mm-dd-yyyy. */
SD *B_FDAT(void)
{
    char *text;
    SD *date = str_tmp(10, &text);
    unsigned year, month, day;

    dev_date(&year, &month, &day);
    put2(text, month);
    text[2] = '-';
    put2(text + 3, day);
    text[5] = '-';
    put2(text + 6, year / 100);
    put2(text + 8, year % 100);
    return date;
}

/* B$FTIM: TIME$, hh:mm:ss. */
SD *B_FTIM(void)
{
    char *text;
    SD *time = str_tmp(8, &text);
    unsigned hour, minute, second;

    dev_time(&hour, &minute, &second);
    put2(text, hour);
    text[2] = ':';
    put2(text + 3, minute);
    text[5] = ':';
    put2(text + 6, second);
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
    if (dev_date_set(year, month, day))
        qb_error(BE_ILLFUN);
    str_tmp_free(text);
}

/* B$STIM: TIME$ = "hh", "hh:mm" or "hh:mm:ss". */
void B_STIM(SD *text)
{
    Scan s;
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
    if (dev_time_set(hours, minutes, seconds))
        qb_error(BE_ILLFUN);
    str_tmp_free(text);
}

/* Days from 1970-01-01 to the first of `month` (1 to 12) of `year`: a civil date's day count, proleptic Gregorian. */
static long days_to_first_of(long year, long month)
{
    long era, year_of_era, day_of_year;

    year -= month <= 2;
    era = (year >= 0 ? year : year - 399) / 400;
    year_of_era = year - era * 400;
    day_of_year = (153 * (month + (month > 2 ? -3 : 9)) + 2) / 5;
    return era * 146097 + year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year - 719468;
}

enum { DAYS_1899_12_30_TO_1970 = 25569 };

/* B$DTSR: DATESERIAL(year, month, day), the days since 1899-12-30 as a DOUBLE whose address is returned. A year of 0
   to 99 is 1900 to 1999; a month or day out of range carries into the next (month 14 is February of the next year, day
   0 the last day of the month before). A date outside the years 100 to 9999 is Illegal function call. */
double *B_DTSR(short year, short month, short day)
{
    static double serial;
    long y = year >= 0 && year <= 99 ? 1900 + year : year, m = (long)month - 1, days;

    if (y < 100 || y > 9999)
        qb_error(BE_ILLFUN);
    y += m >= 0 ? m / 12 : -((11 - m) / 12);
    m = ((m % 12) + 12) % 12 + 1;
    if (y < 100 || y > 9999)
        qb_error(BE_ILLFUN);
    days = days_to_first_of(y, m) + (long)day - 1 + DAYS_1899_12_30_TO_1970;
    serial = (double)days;
    return &serial;
}
