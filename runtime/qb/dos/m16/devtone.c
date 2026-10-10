/* The speaker: timer channel 2 as a square wave, gated by port 61h. */
#include "device.h"

enum {
    TIMER_COMMAND = 0x43,
    TIMER_CHANNEL_2 = 0x42,
    SQUARE_WAVE_2 = 0xB6,
    SPEAKER_PORT = 0x61,
    SPEAKER_ON = 0x03,
    TIMER_CLOCK = 1193182L
};

void dev_tone(unsigned hertz)
{
    unsigned speaker = dev_inb(SPEAKER_PORT);

    if (!hertz) {
        dev_outb(SPEAKER_PORT, speaker & ~SPEAKER_ON);
        return;
    }
    hertz = (unsigned)(TIMER_CLOCK / hertz);
    dev_outb(TIMER_COMMAND, SQUARE_WAVE_2);
    dev_outb(TIMER_CHANNEL_2, hertz & 0xFF);
    dev_outb(TIMER_CHANNEL_2, hertz >> 8);
    dev_outb(SPEAKER_PORT, speaker | SPEAKER_ON);
}
