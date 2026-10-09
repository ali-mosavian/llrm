/* This target's devices: DOS and the BIOS for the clock and the console's
   state, the timer and the speaker port for sound, video memory for the text
   screen. */
#include "device.h"

enum {
    DOS_IOCTL = 0x4400,
    STDOUT_HANDLE = 1,
    DEVICE_BIT = 0x80,
    CONSOLE_OUT_BIT = 0x02,
    CARRY = 1,
    DOS_GET_TIME = 0x2C00,
    MINUTES_PER_HOUR = 60,
    SECONDS_PER_MINUTE = 60,
    HUNDREDTHS = 100,
    TIMER_COMMAND = 0x43,
    TIMER_CHANNEL_2 = 0x42,
    SQUARE_WAVE_2 = 0xB6,
    SPEAKER_PORT = 0x61,
    SPEAKER_ON = 0x03,
    TIMER_CLOCK = 1193182L,
    BIOS_COLUMNS = 0x4A,
    BIOS_MODE = 0x49,
    BIOS_ROWS = 0x84,
    BIOS_CURSOR_SHAPE = 0x60,
    MONO_MODE = 7,
    DEFAULT_ROWS = 25,
    SET_CURSOR_SHAPE = 0x0100,
    SET_CURSOR_POSITION = 0x0200,
    GET_CURSOR_POSITION = 0x0300,
    CURSOR_OFF = 0x2000
};

/* The BIOS data area, 0040h:0000h. */
static const volatile u8 QB_FAR *const bios =
    (const volatile u8 QB_FAR *)0x400UL;

int dev_stdout_is_screen(void)
{
    Regs r;

    r.rax = DOS_IOCTL;
    r.rbx = STDOUT_HANDLE;
    dev_int21(&r);
    return !(r.rflags & CARRY)
           && (r.rdx & (DEVICE_BIT | CONSOLE_OUT_BIT))
              == (DEVICE_BIT | CONSOLE_OUT_BIT);
}

long dev_clock(void)
{
    Regs r;
    long seconds;

    r.rax = DOS_GET_TIME;
    dev_int21(&r);
    seconds = ((r.rcx >> 8) * MINUTES_PER_HOUR + (r.rcx & 0xFF))
              * SECONDS_PER_MINUTE + (r.rdx >> 8);
    return seconds * HUNDREDTHS + (r.rdx & 0xFF);
}

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

static unsigned columns_of(void)
{
    return bios[BIOS_COLUMNS];
}

static u16 QB_FAR *cell(unsigned row, unsigned column)
{
    unsigned long segment = bios[BIOS_MODE] == MONO_MODE ? 0xB000UL : 0xB800UL;

    return (u16 QB_FAR *)(segment << 16) + (row * columns_of() + column);
}

unsigned dev_text_size(void)
{
    unsigned rows = bios[BIOS_ROWS];

    return (rows ? rows + 1 : DEFAULT_ROWS) << 8 | columns_of();
}

unsigned dev_text_cursor(void)
{
    Regs r;

    r.rax = GET_CURSOR_POSITION;
    r.rbx = 0;
    dev_int10(&r);
    return r.rdx;
}

void dev_text_move(unsigned row, unsigned column)
{
    Regs r;

    r.rax = SET_CURSOR_POSITION;
    r.rbx = 0;
    r.rdx = row << 8 | column;
    dev_int10(&r);
}

/* The cursor as the program found it, kept the first time it is hidden. */
void dev_text_cursor_show(int visible)
{
    static unsigned shape;
    Regs r;

    if (!shape) {
        shape = *(const volatile u16 QB_FAR *)(bios + BIOS_CURSOR_SHAPE);
        if (shape & CURSOR_OFF)
            shape = 0;
    }
    r.rax = SET_CURSOR_SHAPE;
    r.rcx = visible ? shape : CURSOR_OFF;
    dev_int10(&r);
}

void dev_text_put(unsigned row, unsigned column, unsigned character,
                  unsigned attribute)
{
    *cell(row, column) = (u16)(attribute << 8 | character);
}

void dev_text_write(unsigned row, unsigned column, const char *text,
                    unsigned count, unsigned attribute)
{
    u16 QB_FAR *at = cell(row, column);

    while (count--)
        *at++ = (u16)(attribute << 8 | (u8)*text++);
}

void dev_text_scroll(unsigned top, unsigned bottom, unsigned lines,
                     unsigned attribute)
{
    unsigned rows = bottom - top + 1, width = columns_of();
    BlockOp op;

    if (lines == 0 || lines >= rows)
        lines = rows;
    op.dst = cell(top, 0);
    op.src = cell(top + lines, 0);
    op.count = (rows - lines) * width * 2;
    op.value = 0;
    dev_move(&op);
    op.dst = cell(bottom + 1 - lines, 0);
    op.count = lines * width;
    op.value = (unsigned)(attribute << 8 | ' ');
    dev_fill(&op);
}
