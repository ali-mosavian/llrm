/* Conversions between strings and numbers: ASC, HEX$, OCT$, MKI$ ... MKD$ and
   CVI ... CVD (QB rt/strfcn.asm, rt/mkcv.asm). */
#include "nhstutil.h"
#include "qb.h"

/* B$FASC: ASC(s$), the first character's code. */
int B_FASC(SD *sd)
{
    int code;

    if (sd->len == 0)
        qb_error(BE_ILLFUN);
    code = (byte)sd->ptr[0];
    str_tmp_free(sd);
    return code;
}

/* The digits of `value`, `bits` to each, most significant first, with no
   leading zeros (but one for zero). */
static SD *digits(unsigned long value, unsigned bits)
{
    char text[11], *at = text + sizeof text, *data;
    SD *result;
    unsigned length;

    do {
        *--at = "0123456789ABCDEF"[value & ((1u << bits) - 1)];
        value >>= bits;
    } while (value);
    length = text + sizeof text - at;
    result = str_tmp(length, &data);
    copy_bytes(data, at, length);
    return result;
}

/* B$FHEX, B$FOCT: HEX$ and OCT$ of a 32-bit value taken as unsigned. */
SD *B_FHEX(unsigned long value)
{
    return digits(value, 4);
}

SD *B_FOCT(unsigned long value)
{
    return digits(value, 3);
}

/* The string of `size` bytes holding `value`'s bytes. */
static SD *made(const void *value, unsigned size)
{
    char *data;
    SD *result = str_tmp(size, &data);

    copy_bytes(data, value, size);
    return result;
}

SD *B_FMKI(short value)
{
    return made(&value, 2);
}

SD *B_FMKL(long value)
{
    return made(&value, 4);
}

SD *B_FMKS(float value)
{
    return made(&value, 4);
}

SD *B_FMKD(double value)
{
    return made(&value, 8);
}

/* The bytes of the first `size` characters of `sd`, in `into`; a shorter
   string is an Illegal function call.  The string is freed if it was a
   temporary. */
static void bytes_of(SD *sd, void *into, unsigned size)
{
    if (sd->len < size)
        qb_error(BE_ILLFUN);
    copy_bytes(into, sd->ptr, size);
    str_tmp_free(sd);
}

int B_FCVI(SD *sd)
{
    int value;

    bytes_of(sd, &value, 2);
    return value;
}

long B_FCVL(SD *sd)
{
    long value;

    bytes_of(sd, &value, 4);
    return value;
}

/* CVS and CVD answer with the address of a value that stays until the next
   conversion, as the other real-valued entries do. */
float *B_FCVS(SD *sd)
{
    static float value;

    bytes_of(sd, &value, 4);
    return &value;
}

double *B_FCVD(SD *sd)
{
    static double value;

    bytes_of(sd, &value, 8);
    return &value;
}
#pragma aux B_FASC "B$FASC"
#pragma aux B_FHEX "B$FHEX"
#pragma aux B_FOCT "B$FOCT"
#pragma aux B_FMKI "B$FMKI"
#pragma aux B_FMKL "B$FMKL"
#pragma aux B_FMKS "B$FMKS"
#pragma aux B_FMKD "B$FMKD"
#pragma aux B_FCVI "B$FCVI"
#pragma aux B_FCVL "B$FCVL"
#pragma aux B_FCVS "B$FCVS"
#pragma aux B_FCVD "B$FCVD"
