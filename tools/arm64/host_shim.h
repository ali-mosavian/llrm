/* The far/huge dialect of the DOS programs, flat; the text screen is a buffer (rt.c). */
extern unsigned char host_text_screen[4000];
#define PHYSICAL_TEXT_SCREEN ((unsigned long)host_text_screen)
