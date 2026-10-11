/* Event trapping: ON TIMER(n) and ON PLAY(n) GOSUB, and their ON, OFF and
   STOP.  QB (rt/gwaevt.asm) jumps into the handler's address at the
   program's line checks; here the compiler numbers the handlers, asks at
   each statement whether an event is due (B$EVAN), and calls the handlers
   of those that are (B$EVPD, B$EVDN) itself. */
#include "device.h"
#include "error.h"
#include "qb.h"

enum { HUNDREDTHS = 100, DAY = 8640000L, LONGEST = 86400 };
enum { DEVICE_TIMER = 1, DEVICE_PLAY = 2 };
enum { OFF, ON, STOP };

typedef struct Trap {
    short handler; /* the compiler's number for the handler, 0 for none */
    byte state;    /* OFF, ON, or STOP: an event is kept, the handler waits */
    byte due;      /* an event happened that the handler has not run for */
    byte running;  /* the handler is running: a second event waits for it */
    long number;   /* TIMER: seconds between events; PLAY: the notes */
    long next;     /* TIMER: when the next is, in hundredths of a second */
    unsigned last; /* PLAY: the notes the last look found */
} Trap;

static Trap timer_trap, play_trap;

extern unsigned music_waiting(void);

static Trap *trap_of(short device)
{
    return device == DEVICE_TIMER  ? &timer_trap
           : device == DEVICE_PLAY ? &play_trap
                                   : 0;
}

/* B$EVAR: ON device(number) GOSUB the handler of that number; 0 takes the
   trap away. */
void B_EVAR(short device, long number, short handler)
{
    Trap *trap = trap_of(device);

    if (!trap ||
        (device == DEVICE_TIMER && handler && (number < 1 || number > LONGEST)))
        qb_error(BE_ILLFUN);
    trap->handler = handler;
    trap->number = number;
}

/* B$EVSW: device ON (mode 1), OFF (0) or STOP (2).  TIMER starts counting
   when it goes on; an off trap forgets its events. */
void B_EVSW(short device, long number, short mode)
{
    Trap *trap = trap_of(device);

    (void)number;
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

/* The events that have happened since the last look. */
static void look(void)
{
    if (timer_trap.state != OFF && timer_trap.handler) {
        long now = dev_clock(), late = (now - timer_trap.next + DAY) % DAY;

        if (late < DAY / 2) {
            timer_trap.due = 1;
            timer_trap.next = (now + timer_trap.number * HUNDREDTHS) % DAY;
        }
    }
    if (play_trap.state != OFF && play_trap.handler) {
        unsigned waiting = music_waiting();

        /* when the notes waiting go from the number to one less */
        if (play_trap.last >= (unsigned long)play_trap.number &&
            waiting < (unsigned long)play_trap.number)
            play_trap.due = 1;
        play_trap.last = waiting;
    }
}

static int ready(const Trap *trap)
{
    return trap->handler && trap->state == ON && trap->due && !trap->running;
}

/* B$EVAN: whether the handler of any event is to run. */
short B_EVAN(void)
{
    look();
    return ready(&timer_trap) || ready(&play_trap);
}

/* B$EVPD: whether the handler of that number is to run; if it is, it is
   running until B$EVDN. */
short B_EVPD(short handler)
{
    Trap *traps[2];
    int at;

    traps[0] = &timer_trap;
    traps[1] = &play_trap;
    for (at = 0; at < 2; at++) {
        if (traps[at]->handler == handler && ready(traps[at])) {
            traps[at]->due = 0;
            traps[at]->running = 1;
            return 1;
        }
    }
    return 0;
}

/* B$EVDN: the handler of that number returned. */
void B_EVDN(short handler)
{
    if (timer_trap.handler == handler)
        timer_trap.running = 0;
    if (play_trap.handler == handler)
        play_trap.running = 0;
}

/* B$FPLY: PLAY(n), the notes waiting in the background music. */
short B_FPLY(short ignored)
{
    (void)ignored;
    return (short)music_waiting();
}
#pragma aux B_EVAR "B$EVAR"
#pragma aux B_EVSW "B$EVSW"
#pragma aux B_EVAN "B$EVAN"
#pragma aux B_EVPD "B$EVPD"
#pragma aux B_EVDN "B$EVDN"
#pragma aux B_FPLY "B$FPLY"
