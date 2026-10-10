/* PLAY (QB rt/gwplays.asm B$SPLY): music written as a string of commands.

   A note is a letter A to G, a sharp (# or +) or a flat (-), a length and dots;
   the other commands set the octave (O, < and >), the length (L), the tempo (T)
   and the style (M).  The notes go to a queue that the clock tick plays through
   the speaker; music in the foreground makes the program wait until the queue is
   empty (the last note has begun), music in the background only while it is
   full. */
#include "device.h"
#include "nhstutil.h"
#include "rtinit.h"

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
    SEMITONES = 12,
    TICKS_PER_MINUTE = 1092390L    /* a minute of the clock's ticks, in thousandths */
};

/* How much of a note sounds before the silence that ends it, in eighths. */
enum Style { NORMAL = 7, LEGATO = 8, STAGGER = 6 };

typedef struct Player {
    const char *at, *end;
    int octave, length, tempo;
    enum Style style;
    byte foreground;
} Player;

static void illegal(void)
{
    qb_error(BE_ILLFUN);
}

/* The next command letter in upper case, or 0 at the end of the string. */
static char next_command(Player *p)
{
    char c;

    while (p->at < p->end && *p->at == ' ')
        p->at++;
    if (p->at == p->end)
        return 0;
    c = *p->at++;
    return c >= 'a' && c <= 'z' ? c - 'a' + 'A' : c;
}

/* The number that follows a command, or -1 if there is none. */
static long number(Player *p)
{
    long value = -1;

    while (p->at < p->end && *p->at == ' ')
        p->at++;
    while (p->at < p->end && *p->at >= '0' && *p->at <= '9') {
        value = (value < 0 ? 0 : value) * 10 + (*p->at++ - '0');
        if (value > 30000)
            illegal();
    }
    return value;
}

/* The music waiting to be played, a ring the clock tick empties: the program
   writes `head` and the tick writes `tail`, so neither needs the other to stop. */
typedef struct Note {
    unsigned hertz;               /* 0 for a rest */
    byte sound, quiet;            /* ticks of tone, then of silence */
} Note;

enum { QUEUE = 19 };   /* one place is left empty: 18 notes wait before a background PLAY does, as with BCOM45 */

static Note queue[QUEUE];
static volatile unsigned head, tail;
static byte tick_started;
static byte sound_left, quiet_left;

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
        if (sound_left)
            dev_tone(next->hertz);
        tail = (tail + 1) % QUEUE;
    }
}
#pragma aux music_tick "B$MUSICTICK"

/* Waits, with the tick running, until `done` is so. */
static void wait_for_room(void)
{
    while ((head + 1) % QUEUE == tail)
        ;
}

static void wait_for_empty(void)
{
    while (tail != head)
        ;
}

/* A time in thousandths of a tick as whole ticks, the part left over kept in
   `owed` for the next time: a run of short notes takes as long as they should,
   though each is a whole number of ticks. */
static byte ticks_of(long thousandths, long *owed)
{
    long whole = (*owed + thousandths) / 1000;

    *owed = *owed + thousandths - whole * 1000;
    return (byte)whole;
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
    static const unsigned octave_three[SEMITONES] = {
        262, 277, 294, 311, 330, 349, 370, 392, 415, 440, 466, 494
    };
    int octave = (note - 1) / SEMITONES - 3;
    unsigned hertz = octave_three[(note - 1) % SEMITONES];

    return octave >= 0 ? hertz << octave : hertz >> -octave;
}

/* Queues a note, or a rest with `note` 0, `time` long. */
static void play_note(Player *p, int note, long time)
{
    static long owed_sound, owed_quiet;
    long sounding = note ? time * p->style / LEGATO : 0;
    Note *at;

    if (!tick_started) {
        dev_ticker_start();
        tick_started = 1;
    }
    wait_for_room();
    at = &queue[head];
    at->hertz = note ? frequency(note) : 0;
    at->sound = ticks_of(sounding, &owed_sound);
    at->quiet = ticks_of(time - sounding, &owed_quiet);
    head = (head + 1) % QUEUE;
    /* with nothing sounding the note starts now, not at the next tick */
    dev_interrupts_off();
    if (!sound_left && !quiet_left)
        music_tick();
    dev_interrupts_on();
}

static int dots(Player *p)
{
    int count = 0;

    while (p->at < p->end && *p->at == '.') {
        p->at++;
        count++;
    }
    return count;
}

static void style(Player *p)
{
    switch (next_command(p)) {
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

    if (p->at < p->end && (*p->at == '#' || *p->at == '+')) {
        p->at++;
        note++;
    } else if (p->at < p->end && *p->at == '-') {
        p->at++;
        note--;
    }
    return note;
}

static void run(Player *p)
{
    char command;
    long n;

    while ((command = next_command(p)) != 0) {
        if (command >= 'A' && command <= 'G') {
            int note = named_note(p, command);

            n = number(p);
            if (n == 0 || n > LONGEST_NOTE)
                illegal();
            play_note(p, note, duration(p, n < 0 ? p->length : (int)n,
                                        dots(p)));
        } else if (command == 'N') {
            n = number(p);
            if (n < 0 || n > HIGHEST_NOTE)
                illegal();
            play_note(p, (int)n, duration(p, p->length, dots(p)));
        } else if (command == 'P') {
            n = number(p);
            if (n < 1 || n > LONGEST_NOTE)
                illegal();
            play_note(p, 0, duration(p, (int)n, dots(p)));
        } else if (command == 'O') {
            n = number(p);
            if (n < LOWEST_OCTAVE || n > HIGHEST_OCTAVE)
                illegal();
            p->octave = (int)n;
        } else if (command == '<' || command == '>') {
            p->octave += command == '>' ? 1 : -1;
            if (p->octave < LOWEST_OCTAVE || p->octave > HIGHEST_OCTAVE)
                illegal();
        } else if (command == 'L') {
            n = number(p);
            if (n < 1 || n > LONGEST_NOTE)
                illegal();
            p->length = (int)n;
        } else if (command == 'T') {
            n = number(p);
            if (n < LOWEST_TEMPO || n > HIGHEST_TEMPO)
                illegal();
            p->tempo = (int)n;
        } else if (command == 'M') {
            style(p);
        } else {
            illegal();
        }
    }
}

/* B$SPLY: PLAY with the string of commands.  What the commands set (the octave,
   the length, the tempo, the style and foreground or background) stays set for
   the next PLAY.  Music in the foreground has the program wait until the last
   note has begun. */
void B_SPLY(SD *commands)
{
    static Player player = { 0, 0, DEFAULT_OCTAVE, DEFAULT_LENGTH, DEFAULT_TEMPO, NORMAL, 1 };

    player.at = commands->ptr;
    player.end = commands->ptr + commands->len;
    run(&player);
    str_tmp_free(commands);
    if (player.foreground)
        wait_for_empty();
}
#pragma aux B_SPLY "B$SPLY"

/* The program has ended: what is queued is not played, and the speaker is quiet. */
static void music_stop(void)
{
    tail = head;
    sound_left = quiet_left = 0;
    dev_tone(0);
}

static Comp comp = { 0, C_SN, { 0, 0, 0, 0, music_stop } };

#define XI_FN play_xinit
#include "xi.h"
void play_xinit(void)
{
    qb_comp_add(&comp);
}
