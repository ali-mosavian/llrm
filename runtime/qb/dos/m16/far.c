/* Paragraph-addressed memory for the far heap and for arrays that are not in
   DGROUP. */
#include "far.h"

enum { CHUNK = 0x1000, WORDS_PER_PARA = 8 };

static char marker;

u16 dgroup_segment(void)
{
    return (u16)((unsigned long)(char __far *)&marker >> 16);
}

FarWords far_words(u16 segment)
{
    return (FarWords)((unsigned long)segment << 16);
}

void far_clear(u16 segment, u16 paras)
{
    while (paras) {
        u16 chunk = paras < CHUNK ? paras : CHUNK;
        u16 left = chunk * WORDS_PER_PARA;
        FarWords at = far_words(segment);

        while (left--)
            *at++ = 0;
        segment += chunk;
        paras -= chunk;
    }
}

void far_clear_bytes(u16 segment, u16 offset, unsigned long bytes)
{
    while (bytes) {
        u16 piece = bytes < 0x8000 ? bytes : 0x8000, left = (piece + 1) / 2;
        FarWords at = (FarWords)(((unsigned long)segment << 16) | offset);

        while (left--)
            *at++ = 0;
        offset += piece;
        segment += offset >> 4;
        offset &= 15;
        bytes -= piece;
    }
}

/* A move up starts from the end, so a move over itself reads each u16 before it
   writes it. */
void far_move(u16 from, u16 to, u16 paras)
{
    int up = to > from;

    while (paras) {
        u16 chunk = paras < CHUNK ? paras : CHUNK;
        u16 left = chunk * WORDS_PER_PARA;
        u16 offset = up ? paras - chunk : 0;
        FarWords source = far_words(from + offset);
        FarWords target = far_words(to + offset);

        if (up) {
            source += left;
            target += left;
            while (left--)
                *--target = *--source;
        } else {
            while (left--)
                *target++ = *source++;
        }
        paras -= chunk;
        if (!up) {
            from += chunk;
            to += chunk;
        }
    }
}
