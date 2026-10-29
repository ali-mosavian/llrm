/* PLAY (QB rt/gwplays.asm B$SPLY): music written as a string of commands.

   A note is a letter A to G, a sharp (# or +) or a flat (-), a length and dots;
   the other commands set the octave (O, < and >), the length (L), the tempo (T)
   and the style (M).  The notes go to a queue that the clock tick plays through
   the speaker; music in the foreground makes the program wait until the queue
   is empty (the last note has begun), music in the background only while it is
   full. */
#include "dev.h"
#include "nhstutil.h"
#include "varptr.h"
#include "rtinit.h"

/* a minute of the clock's ticks, in thousandths */
#define TICKS_PER_MINUTE 1092390L
enum {
    LOWEST_OCTAVE = 0,
    HIGHEST_OCTAVE = 6,
    DEFAULT_OCTAVE = 4,
    DEFAULT_LENGTH = 4,
    DEFAULT_TEMPO = 120,
    LOWEST_TEMPO = 32,
    HIGHEST_TEMPO = 255,
    LONGEST_NOTE = 64,
    HIGHEST_NOTE = 84,
    SEMITONES = 12
};

/* How much of a note sounds before the silence that ends it, in eighths. */
enum Style { NORMAL = 7, LEGATO = 8, STAGGER = 6 };

typedef struct Player {
    Macro m;
    int octave, length, tempo;
    enum Style style;
    byte foreground;
} Player;

static void illegal(void)
{
    qb_error(BE_ILLFUN);
}

/* The number that follows a command, or -1 if there is none. */
static long number(Player *p)
{
    long value = -1;

    while (p->m.at < p->m.end && *p->m.at == ' ')
        p->m.at++;
    while (p->m.at < p->m.end && *p->m.at >= '0' && *p->m.at <= '9') {
        value = (value < 0 ? 0 : value) * 10 + (*p->m.at++ - '0');
        if (value > 30000)
            illegal();
    }
    if (value < 0)
        macro_equals(&p->m, &value);
    return value;
}

/* The music waiting to be played, a ring the clock tick empties: the program
   writes `head` and the tick writes `tail`, so neither needs the other to stop.
 */
typedef struct Note {
    unsigned hertz;        /* 0 for a rest */
    unsigned sound, quiet; /* ticks of tone, then of silence */
} Note;

enum {
    QUEUE = 19
}; /* one place is left empty: 18 notes wait before a background PLAY does, as
      with BCOM45 */

static Note queue[QUEUE];
static volatile unsigned head, tail;
static byte tick_started;
static volatile unsigned sound_left, quiet_left;

#pragma aux music_tick "B$MUSICTICK"
#pragma aux B_SPLY "B$SPLY"
#pragma aux B_SOND "B$SOND"

/* The clock tick: counts the note out, and starts the next. */
void music_tick(void)
{
    if (sound_left && --sound_left == 0)
        dev_tone(0);
    else if (!sound_left && quiet_left)
        quiet_left--;
    /* a note too short to last a tick is passed over */
    while (!sound_left && !quiet_left && tail != head) {
        const Note *next = &queue[tail];

        sound_left = next->sound;
        quiet_left = next->quiet;
        if (sound_left && next->hertz)
            dev_tone(next->hertz);
        tail = (tail + 1) % QUEUE;
    }
}


/* The notes waiting: PLAY(n) and ON PLAY (events.c). */
unsigned music_waiting(void)
{
    return (head + QUEUE - tail) % QUEUE;
}

/* Waits, with the tick running, until `done` is so. */
static void wait_for_room(void)
{
    while ((head + 1) % QUEUE == tail)
        ;
}

/* A time in thousandths of a tick as whole ticks, the part left over kept in
   `owed` for the next time: a run of short notes takes as long as they should,
   though each is a whole number of ticks. */
static unsigned ticks_of(long thousandths, long *owed)
{
    long whole = (*owed + thousandths) / 1000;

    *owed = *owed + thousandths - whole * 1000;
    return (unsigned)whole;
}

/* A note of `length`th of a whole note and `dots` dots, in thousandths of a
   tick. */
static long duration(const Player *p, int length, int dots)
{
    long whole = 4L * TICKS_PER_MINUTE / p->tempo;
    long time = whole / length, extra = time;

    while (dots--) {
        extra /= 2;
        time += extra;
    }
    return time;
}

static unsigned frequency(int note)
{
    static const unsigned octave_three[SEMITONES] = {262, 277, 294, 311, 330, 349,
                                                     370, 392, 415, 440, 466, 494};
    int octave = (note - 1) / SEMITONES - 3;
    unsigned hertz = octave_three[(note - 1) % SEMITONES];

    return octave >= 0 ? hertz << octave : hertz >> -octave;
}

/* Queues a note, or a rest with `note` 0, `time` long. */
static void play_note(Player *p, int note, long time)
{
    static long owed_sound, owed_quiet;
    long sounding = note ? time * p->style / LEGATO : time;
    Note *at;

    if (!tick_started) {
        dev_tick_start();
        tick_started = 1;
    }
    /* in the foreground a note waits for the queue ahead of it and for the
       note sounding to end, not for the silence that follows it; a rest counts
       as sound */
    if (p->foreground)
        while (tail != head || sound_left)
            ;
    wait_for_room();
    at = &queue[head];
    at->hertz = note ? frequency(note) : 0;
    at->sound = ticks_of(sounding, &owed_sound);
    at->quiet = ticks_of(time - sounding, &owed_quiet);
    head = (head + 1) % QUEUE;
}

static int dots(Player *p)
{
    int count = 0;

    while (p->m.at < p->m.end && *p->m.at == '.') {
        p->m.at++;
        count++;
    }
    return count;
}

static void style(Player *p)
{
    switch (macro_next(&p->m)) {
    case 'F':
        p->foreground = 1;
        break;
    case 'B':
        p->foreground = 0;
        break;
    case 'N':
        p->style = NORMAL;
        break;
    case 'L':
        p->style = LEGATO;
        break;
    case 'S':
        p->style = STAGGER;
        break;
    default:
        illegal();
    }
}

/* A letter note: its semitone above C, with a sharp or a flat. */
static int named_note(Player *p, char letter)
{
    static const int semitone[7] = {9, 11, 0, 2, 4, 5, 7};
    int note = p->octave * SEMITONES + semitone[letter - 'A'] + 1;

    if (p->m.at < p->m.end && (*p->m.at == '#' || *p->m.at == '+')) {
        p->m.at++;
        note++;
    } else if (p->m.at < p->m.end && *p->m.at == '-') {
        p->m.at++;
        note--;
    }
    return note;
}

static void run(Player *p)
{
    char command;
    long n;

    while ((command = macro_next(&p->m)) != 0) {
        switch (command) {
        case 'A':
        case 'B':
        case 'C':
        case 'D':
        case 'E':
        case 'F':
        case 'G': {
            int note = named_note(p, command);

            n = number(p);
            if (n == 0 || n > LONGEST_NOTE)
                illegal();
            play_note(p, note, duration(p, n < 0 ? p->length : (int)n, dots(p)));
            break;
        }
        case 'N':
            n = number(p);
            if (n < 0 || n > HIGHEST_NOTE)
                illegal();
            play_note(p, (int)n, duration(p, p->length, dots(p)));
            break;
        case 'P':
            n = number(p);
            if (n < 1 || n > LONGEST_NOTE)
                illegal();
            play_note(p, 0, duration(p, (int)n, dots(p)));
            break;
        case 'O':
            n = number(p);
            if (n < LOWEST_OCTAVE || n > HIGHEST_OCTAVE)
                illegal();
            p->octave = (int)n;
            break;
        case '<':
        case '>':
            p->octave += command == '>' ? 1 : -1;
            if (p->octave < LOWEST_OCTAVE || p->octave > HIGHEST_OCTAVE)
                illegal();
            break;
        case 'L':
            n = number(p);
            if (n < 1 || n > LONGEST_NOTE)
                illegal();
            p->length = (int)n;
            break;
        case 'T':
            n = number(p);
            if (n < LOWEST_TEMPO || n > HIGHEST_TEMPO)
                illegal();
            p->tempo = (int)n;
            break;
        case 'M':
            style(p);
            break;
        case 'X': {
            /* the commands of a string variable, then on with these */
            Macro outer;

            macro_enter(&p->m, &outer);
            run(p);
            macro_leave(&p->m, &outer);
            break;
        }
        default:
            illegal();
        }
    }
}

/* B$SPLY: PLAY with the string of commands.  What the commands set (the octave,
   the length, the tempo, the style and foreground or background) stays set for
   the next PLAY.  Music in the foreground has each note wait for the one
   sounding to end. */
/* What PLAY keeps from one use to the next, and SOUND reads. */
static Player player = {{0, 0, 0}, DEFAULT_OCTAVE, DEFAULT_LENGTH, DEFAULT_TEMPO, NORMAL, 1};

void B_SPLY(SD *commands)
{
    macro_begin(&player.m, commands);
    run(&player);
    str_tmp_free(commands);
}


/* B$SOND: SOUND frequency, duration in clock ticks (18.2 a second).  In
   foreground music a note waits for the one sounding to end; in background
   music it joins the queue and the program goes on.  The note starts at the
   next tick.  A duration of 0 silences what is sounding. */
void B_SOND(short hertz, float duration)
{
    long ticks;
    Note *at;

    if (duration < 0 || duration > 65535.5f)
        illegal();
    ticks = (long)(duration + 0.5f);
    if (ticks == 0) {
        dev_tick_hold();
        tail = head;
        sound_left = quiet_left = 0;
        dev_tone(0);
        dev_tick_release();
        return;
    }
    if (hertz < 37)
        illegal();
    if (!tick_started) {
        dev_tick_start();
        tick_started = 1;
    }
    if (player.foreground)
        while (tail != head || sound_left)
            ;
    wait_for_room();
    at = &queue[head];
    at->hertz = (unsigned)hertz;
    at->sound = (unsigned)ticks;
    at->quiet = 0;
    head = (head + 1) % QUEUE;
}


/* The program has ended: what is queued is not played, and the speaker is
 * quiet. */
static void music_stop(void)
{
    tail = head;
    sound_left = quiet_left = 0;
    dev_tone(0);
    if (tick_started)
        dev_tick_stop();
}

static Comp comp = {0, C_SN, {0, 0, 0, 0, music_stop}};

#define XI_FN play_xinit
#include "xi.h"

void play_xinit(void)
{
    qb_comp_add(&comp);
}
