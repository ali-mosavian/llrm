/* The runtime entries whose arguments QB pushes as a block (varargs.asm at m32): the compiler for a host (llrm-qb --arm64)
   calls them with the block's words as variable arguments, in the order of the block in memory: B$DDIM, B$RDIM and
   B$RDMP (the descriptor, rank and features, element size, then upper and lower bound of each dimension), B$CLOS (count,
   channels), and B$COLR, B$LOCT and B$CSCN (count, then a flag and a value for each argument). The words are 16-bit
   INTEGERs, passed in 32-bit registers or slots with nothing said of the upper half. */
#include <stdarg.h>

#include "array.h"
#include "file.h"

void screen_color(const int *block);
void screen_locate(const int *block);
void screen_mode(const int *block);

enum { MAX_RANK = 60, MAX_SLOTS = 32 };

static void dim(enum DimMode mode, AD *ad, va_list words)
{
    struct {
        AD *ad;
        int rank_and_features;
        int element;
        struct {
            int upper, lower;
        } bounds[MAX_RANK];
    } call;
    unsigned rank, at;

    call.ad = ad;
    call.rank_and_features = (unsigned short)va_arg(words, int);
    call.element = (unsigned short)va_arg(words, int);
    rank = (unsigned)call.rank_and_features & 0xFF;
    for (at = 0; at < rank && at < MAX_RANK; at++) {
        call.bounds[at].upper = (short)va_arg(words, int);
        call.bounds[at].lower = (short)va_arg(words, int);
    }
    array_dim(mode, (const DimCall *)&call);
}

void B_DDIM(AD *ad, ...)
{
    va_list words;

    va_start(words, ad);
    dim(DIM_ALLOCATE, ad, words);
    va_end(words);
}

void B_RDIM(AD *ad, ...)
{
    va_list words;

    va_start(words, ad);
    dim(DIM_REALLOCATE, ad, words);
    va_end(words);
}

void B_RDMP(AD *ad, ...)
{
    va_list words;

    va_start(words, ad);
    dim(DIM_PRESERVE, ad, words);
    va_end(words);
}

/* The count and the words after it, as the C routines read them: a block of ints. */
static void block(int first, va_list words, int out[MAX_SLOTS + 1])
{
    unsigned count = (unsigned short)first, at;

    out[0] = (int)count;
    for (at = 1; at <= count && at <= MAX_SLOTS; at++)
        out[at] = (short)va_arg(words, int);
}

void B_CLOS(int count, ...)
{
    va_list words;
    int channels[MAX_SLOTS + 1];

    va_start(words, count);
    block(count, words, channels);
    va_end(words);
    file_close(channels + 1, (unsigned)channels[0]);
}

void B_COLR(int count, ...)
{
    va_list words;
    int slots[MAX_SLOTS + 1];

    va_start(words, count);
    block(count, words, slots);
    va_end(words);
    screen_color(slots);
}

void B_LOCT(int count, ...)
{
    va_list words;
    int slots[MAX_SLOTS + 1];

    va_start(words, count);
    block(count, words, slots);
    va_end(words);
    screen_locate(slots);
}

void B_CSCN(int count, ...)
{
    va_list words;
    int slots[MAX_SLOTS + 1];

    va_start(words, count);
    block(count, words, slots);
    va_end(words);
    screen_mode(slots);
}
