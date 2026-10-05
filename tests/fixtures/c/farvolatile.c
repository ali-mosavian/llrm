/* A volatile far store to video memory (B800) writes only its own bytes:
   *ch and *at stay in registers. At 0x1234, which may be the program's
   data, they are read again. */
#define FILL(name, address)                                                  \
    void name(int *ch, int *at)                                              \
    {                                                                        \
        unsigned char volatile far *screen = (unsigned char volatile far *)address; \
        int o;                                                               \
        for (o = 0; o < 4000; o += 2) {                                      \
            screen[o] = *ch + (o & 15);                                      \
            screen[o + 1] = *at;                                             \
        }                                                                    \
    }

FILL(video, 0xB8000000L)
FILL(conventional, 0x12340000L)
