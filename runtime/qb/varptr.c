/* VARPTR$ (QB rt/strfcn.asm B$VARP) and SADD (B$SADD), and the macro
   languages' reading of a VARPTR$ string. */
#include "error.h"
#include "nhstutil.h"
#include "qb.h"
#include "varptr.h"

enum { TYPE_INTEGER = 2, TYPE_STRING = 3, TYPE_SINGLE = 4, TYPE_DOUBLE = 8 };

#pragma aux B_VARP "B$VARP"
#pragma aux B_SADD "B$SADD"

/* B$VARP: the string of a variable's type byte and its address. */
SD *B_VARP(unsigned address, short type)
{
    char *text;
    SD *made = str_tmp(1 + sizeof address, &text);
    unsigned i;

    text[0] = (char)type;
    for (i = 0; i < sizeof address; i++)
        text[1 + i] = (char)(address >> (8 * i));
    return made;
}

/* B$SADD: where the characters of a string are. */
int B_SADD(SD *string)
{
    return (int)(unsigned)(unsigned long)string->ptr;
}

/* A number rounded to a whole one, a half going to the even. */
static long rounded(double value)
{
    long whole = (long)value;
    double part = value - whole;

    if (part > 0.5 || (part == 0.5 && (whole & 1)))
        return whole + 1;
    if (part < -0.5 || (part == -0.5 && (whole & 1)))
        return whole - 1;
    return whole;
}

static void *address_at(const char **at, const char *end, int *type)
{
    unsigned address = 0, i;

    if ((unsigned long)(end - *at) < 1 + sizeof address)
        qb_error(BE_ILLFUN);
    *type = (unsigned char)(*at)[0];
    for (i = 0; i < sizeof address; i++)
        address |= (unsigned)(unsigned char)(*at)[1 + i] << (8 * i);
    *at += 1 + sizeof address;
    return (void *)(unsigned long)address;
}

long varptr_integer(const char **at, const char *end)
{
    int type;
    void *variable = address_at(at, end, &type);

    switch (type) {
    case TYPE_INTEGER:
        return *(short *)variable;
    case TYPE_SINGLE:
        return rounded(*(float *)variable);
    case TYPE_DOUBLE:
        return rounded(*(double *)variable);
    default:
        qb_error(BE_ILLFUN);
    }
    return 0;
}

const SD *varptr_string(const char **at, const char *end)
{
    int type;
    void *variable = address_at(at, end, &type);

    if (type != TYPE_STRING)
        qb_error(BE_ILLFUN);
    return (const SD *)variable;
}


enum { NESTING = 16 };

void macro_begin(Macro *m, const SD *commands)
{
    m->at = commands->ptr;
    m->end = commands->ptr + commands->len;
    m->depth = 0;
}

char macro_next(Macro *m)
{
    while (m->at < m->end && (*m->at == ' ' || *m->at == ';'))
        m->at++;
    if (m->at >= m->end)
        return 0;
    return *m->at >= 'a' && *m->at <= 'z' ? (char)(*m->at++ - 'a' + 'A') : *m->at++;
}

int macro_equals(Macro *m, long *value)
{
    if (m->at >= m->end || *m->at != '=')
        return 0;
    m->at++;
    *value = varptr_integer(&m->at, m->end);
    return 1;
}

void macro_enter(Macro *m, Macro *outer)
{
    const SD *text = varptr_string(&m->at, m->end);

    *outer = *m;
    if (++m->depth > NESTING)
        qb_error(BE_ILLFUN);
    m->at = text->ptr;
    m->end = text->ptr + text->len;
}

void macro_leave(Macro *m, const Macro *outer)
{
    m->at = outer->at;
    m->end = outer->end;
    m->depth--;
}
