/* The portable runtime on the host, for the test of the span fills: video memory is two arrays. */
#ifndef QB_PLATFORM_H
#define QB_PLATFORM_H
#include <string.h>
typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
#define QB_FAR
extern unsigned char vga_ram[0x10000], cga_ram[0x4000];
#define QB_VIDEO_MEMORY(segment) ((void *)((segment) == 0xB800 ? cga_ram : vga_ram))
#define QB_REAL_POINTER(segment, offset) ((void *)0)
#endif
