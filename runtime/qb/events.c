/* Event trapping: ON TIMER(n), ON PLAY(n) and ON KEY(n) GOSUB, and their ON,
   OFF and STOP.  QB (rt/gwaevt.asm) jumps into the handler's address at the
   program's line checks; here the compiler numbers the handlers, asks at
   each statement whether an event is due (B$EVAN), and calls the handlers
   of those that are (B$EVPD, B$EVDN) itself. */
#include "console.h"
#include "dev.h"
#include "error.h"
#include "qb.h"

#define DAY 8640000L
#define LONGEST 86400L
enum { HUNDREDTHS = 100 };

enum {
    DEVICE_TIMER = 1,
    DEVICE_PLAY = 2,
    DEVICE_KEY = 3,
    DEVICE_FIRST_ABSENT = 4,
    DEVICE_LAST = 8
};

enum { OFF, ON, STOP };

enum { KEYS = 32, ABSENT = DEVICE_LAST - DEVICE_FIRST_ABSENT + 1, TRAPS = KEYS + 1 + ABSENT };

typedef struct Trap {
    short handler; /* the compiler's number for the handler, 0 for none */
    byte state;    /* OFF, ON, or STOP: an event is kept, the handler waits */
    byte due;      /* an event happened that the handler has not run for */
    byte running;  /* the handler is running: a second event waits for it */
    long number;   /* TIMER: seconds between events; PLAY: the notes */
    long next;     /* TIMER: when the next is, in hundredths of a second */
    unsigned last; /* PLAY: the notes the last look found */
} Trap;

/* The timer, the music, KEY(1) to KEY(31), then the joystick, serial port,
   light pen, user event and signal, of which this machine has none: they take
   their statements and never have an event. */
static Trap traps[TRAPS];

/* Whether any KEY(n) has a handler: until one has, the keyboard is left alone.
   */
static int keys_armed;

/* The scan codes of KEY(1) to KEY(14), F11 and F12 (30 and 31) */
static const byte key_scan[KEYS] = {
    0, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x48, 0x4B, 0x4D, 0x50, 0,
    0, 0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0x85, 0x86};

extern unsigned music_waiting(void);

static int take_keys(void);

static Trap *trap_of(short device, long number)
{
    if (device == DEVICE_TIMER)
        return &traps[0];
    if (device == DEVICE_PLAY)
        return &traps[1];
    if (device == DEVICE_KEY && number >= 1 && number < KEYS && key_scan[number])
        return &traps[1 + number];
    if (device >= DEVICE_FIRST_ABSENT && device <= DEVICE_LAST)
        return &traps[KEYS + 1 + device - DEVICE_FIRST_ABSENT];
    return 0;
}

#pragma aux B_STIK "B$STIK"
#pragma aux B_FSTG "B$FSTG"
#pragma aux B_FPEN "B$FPEN"
#pragma aux B_EVAR "B$EVAR"
#pragma aux B_EVSW "B$EVSW"
#pragma aux B_EVAN "B$EVAN"
#pragma aux B_EVPD "B$EVPD"
#pragma aux B_EVDN "B$EVDN"
#pragma aux B_FPLY "B$FPLY"

/* CLEAR switches every trap off and forgets the events it kept; the handlers stay (clear.c). */
void events_clear(void)
{
    unsigned at;

    for (at = 0; at < TRAPS; at++) {
        traps[at].state = OFF;
        traps[at].due = 0;
        traps[at].running = 0;
    }
}

/* B$EVAR: ON device(number) GOSUB the handler of that number; 0 takes the
   trap away. */
void B_EVAR(short device, long number, short handler)
{
    Trap *trap = trap_of(device, number);

    if (!trap || (device == DEVICE_TIMER && handler && (number < 1 || number > LONGEST)))
        qb_error(BE_ILLFUN);
    trap->handler = handler;
    trap->number = number;
    keys_armed = 0;
    for (device = 2; device <= KEYS; device++)
        keys_armed |= traps[device].handler != 0;
    cn_key_hook = keys_armed ? take_keys : 0;
}

/* B$EVSW: device ON (mode 1), OFF (0) or STOP (2).  TIMER starts counting
   when it goes on; an off trap forgets its events. */
void B_EVSW(short device, long number, short mode)
{
    Trap *trap = trap_of(device, number);

    if (!trap)
        qb_error(BE_ILLFUN);
    if (mode == ON && trap->state == OFF) {
        trap->next = (dev_clock() + trap->number * HUNDREDTHS) % DAY;
        trap->last = device == DEVICE_PLAY ? music_waiting() : 0;
    }
    if (mode == OFF)
        trap->due = 0;
    trap->state = (byte)mode;
}

/* Takes a trapped key that is next in the keyboard buffer: its event is due
   and the program never reads the key.  Returns 0 when the next key is not
   one, or there is none. */
static int trapped_key(void)
{
    int scan = dev_key_peek_scan(), key;

    if (scan < 0)
        return 0;
    for (key = 1; key < KEYS; key++) {
        Trap *trap = &traps[1 + key];

        if (key_scan[key] == scan && trap->handler && trap->state != OFF) {
            dev_key_discard();
            trap->due = 1;
            return 1;
        }
    }
    return 0;
}

/* Takes the trapped keys waiting; whether there were any. */
static int take_keys(void)
{
    int took = 0;

    while (trapped_key())
        took = 1;
    return took;
}

/* The events that have happened since the last look. */
static void look(void)
{
    Trap *timer = &traps[0], *play = &traps[1];

    if (timer->state != OFF && timer->handler) {
        long now = dev_clock(), late = (now - timer->next + DAY) % DAY;

        if (late < DAY / 2) {
            timer->due = 1;
            timer->next = (now + timer->number * HUNDREDTHS) % DAY;
        }
    }
    if (play->state != OFF && play->handler) {
        unsigned waiting = music_waiting();

        /* when the notes waiting go from the number to one less */
        if (play->last >= (unsigned long)play->number && waiting < (unsigned long)play->number)
            play->due = 1;
        play->last = waiting;
    }
    if (keys_armed)
        take_keys();
}

static int ready(const Trap *trap)
{
    return trap->handler && trap->state == ON && trap->due && !trap->running;
}

/* B$EVAN: whether the handler of any event is to run. */
short B_EVAN(void)
{
    int at;

    look();
    for (at = 0; at < TRAPS; at++)
        if (ready(&traps[at]))
            return 1;
    return 0;
}

/* B$EVPD: whether the handler of that number is to run; if it is, it is
   running until B$EVDN. */
short B_EVPD(short handler)
{
    int at;

    for (at = 0; at < TRAPS; at++) {
        if (traps[at].handler == handler && ready(&traps[at])) {
            traps[at].due = 0;
            traps[at].running = 1;
            return 1;
        }
    }
    return 0;
}

/* B$EVDN: the handler of that number returned. */
void B_EVDN(short handler)
{
    int at;

    for (at = 0; at < TRAPS; at++)
        if (traps[at].handler == handler)
            traps[at].running = 0;
}

/* B$FPLY: PLAY(n), the notes waiting in the background music. */
short B_FPLY(short ignored)
{
    (void)ignored;
    return (short)music_waiting();
}

/* B$STIK, B$FSTG and B$FPEN: STICK(n), STRIG(n) and PEN(n) of a machine
   without a joystick or a light pen: nothing moves and nothing is pressed. */
short B_STIK(short which)
{
    (void)which;
    return 0;
}

short B_FSTG(short which)
{
    (void)which;
    return 0;
}

short B_FPEN(short which)
{
    (void)which;
    return 0;
}
