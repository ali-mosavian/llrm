/* The PC's low memory as a program sees it through DEF SEG, PEEK and POKE: the compiler computes the linear address
   (segment * 16 + offset) and asks for its place here, since a 64-bit host has no memory there. The image is a
   megabyte of plain memory; the screen's own memory is not in it. */
#include <stdint.h>

enum { REAL_BYTES = 1 << 20, BIOS_KEYBOARD_FLAGS = 0x417 };

static uint8_t real_memory[REAL_BYTES + 16];

void *B_REAL(uintptr_t linear)
{
    return real_memory + (linear % REAL_BYTES);
}

/* DEF SEG's segment (QB's b$seg): the compiled code stores it, PEEK and POKE add it. */
unsigned short b_seg;
