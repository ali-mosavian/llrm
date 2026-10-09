/* PLAY (QB rt/gwplays.asm B$SPLY): music written as a string of commands.

   A note is a letter A to G, a sharp (# or +) or a flat (-), a length and dots;
   the other commands set the octave (O, < and >), the length (L), the tempo (T)
   and the style (M).  Music in the background (MB) is accepted and not played;
   music in the foreground is played through the speaker, and the program waits
   for it. */
#include "llrm_os.h"
#include "nhstutil.h"

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
    HUNDREDTHS_PER_MINUTE = 6000,
    DAY = 8640000L
};

/* How much of a note sounds before the silence that ends it, in eighths. */
enum Style { NORMAL = 7, LEGATO = 8, STAGGER = 6 };

typedef struct Player {
    const char *at, *end;
    int octave, length, tempo;
    enum Style style;
    int foreground;
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

    while (p->at < p->end && *p->at >= '0' && *p->at <= '9') {
        value = (value < 0 ? 0 : value) * 10 + (*p->at++ - '0');
        if (value > 30000)
            illegal();
    }
    return value;
}

/* Waits `hundredths` of a second. */
static void wait(long hundredths)
{
    long start = llrm_os_clock_hundredths();

    while ((llrm_os_clock_hundredths() - start + DAY) % DAY < hundredths)
        ;
}

/* A note of `length`th of a whole note and `dots` dots, in hundredths. */
static long duration(const Player *p, int length, int dots)
{
    long whole = 4L * HUNDREDTHS_PER_MINUTE / p->tempo;
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

static void play_note(Player *p, int note, long time)
{
    long sounding = time * p->style / LEGATO;

    if (!p->foreground)
        return;
    llrm_os_speaker_tone(note ? frequency(note) : 0);
    wait(note ? sounding : time);
    llrm_os_speaker_tone(0);
    if (note)
        wait(time - sounding);
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

/* B$SPLY: PLAY with the string of commands. */
void B_SPLY(SD *commands)
{
    Player player;

    player.at = commands->ptr;
    player.end = commands->ptr + commands->len;
    player.octave = DEFAULT_OCTAVE;
    player.length = DEFAULT_LENGTH;
    player.tempo = DEFAULT_TEMPO;
    player.style = NORMAL;
    player.foreground = 1;
    run(&player);
    str_tmp_free(commands);
}
#pragma aux B_SPLY "B$SPLY"
