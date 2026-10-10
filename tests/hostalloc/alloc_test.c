/* Random string operations against the string allocator (runtime/qb/nhstutil.c), on the host: each live string keeps
   the bytes written to it, the entries tile the space, and a free entry is never one a live string owns.  Exits 0, or 1
   with the step that broke it. */
#include <stdio.h>
#include <stdlib.h>
#include "../../runtime/qb/nhstutil.c"
#include "../../runtime/qb/stcore.c"

void qb_error(unsigned n) { fprintf(stderr, "qb_error %u\n", n); exit(3); }
void copy_bytes(char *to, const char *from, unsigned n) { while (n--) *to++ = *from++; }
int lh_take_from_strings(void) { return 0; }
void lh_give_free_to_strings(void) {}
void *lh_alloc(uword b, enum LhType t, void *o, byte f) { (void)b; (void)t; (void)o; (void)f; return 0; }
void lh_free(void *d) { (void)d; }

enum { LIVE = 64, POOL = 1 << 16 };
static SD names[LIVE];
static unsigned char pattern[LIVE];

static unsigned rng = 12345;
static unsigned next(void) { rng = rng * 1103515245u + 12345u; return (rng >> 8) & 0xFFFFFF; }

static void fail(const char *what, int step)
{
    fprintf(stderr, "step %d: %s\n", step, what);
    exit(1);
}

static void check(int step)
{
    StrEntry *entry;
    unsigned i;
    unsigned long seen = 0;

    for (entry = str_first; !AT_END(entry); entry = FOLLOWING(entry)) {
        if (!IS_FREE(entry)) {
            SD *owner = OWNER_OF(entry);
            if (owner->ptr != DATA_OF(entry))
                fail("an owner does not point at its entry", step);
        }
        seen += ENTRY_BYTES(entry);
    }
    if ((char *)str_first + seen != str_end)
        fail("the entries do not tile the space", step);
    for (i = 0; i < LIVE; i++) {
        unsigned j;
        for (j = 0; j < names[i].len; j++)
            if ((unsigned char)names[i].ptr[j] != (unsigned char)(pattern[i] + j))
                fail("a live string changed", step);
    }
}

static SD *make(unsigned len, unsigned char base)
{
    char *data;
    SD *sd = str_tmp(len, &data);
    unsigned j;

    for (j = 0; j < len; j++)
        data[j] = (char)(base + j);
    return sd;
}

int main(int argc, char **argv)
{
    static char pool[POOL];
    int step;
    unsigned seed = argc > 1 ? (unsigned)atoi(argv[1]) : 1;

    rng = seed;
    str_init(pool, pool + POOL);
    for (step = 0; step < 200000; step++) {
        unsigned i = next() % LIVE, len = next() % 40;

        switch (next() % 8) {
        case 0:
        case 1: {                      /* a$ = temporary */
            SD *t = make(len, (unsigned char)(i * 7 + step));
            pattern[i] = (unsigned char)(i * 7 + step);
            str_assign(t, &names[i]);
            break;
        }
        case 2:                        /* a$ = LEFT$(b$, n) */
            {
                unsigned k = next() % LIVE, n = next() % 8;
                SD *t = str_tmp_copy(&names[k], 0, n < names[k].len ? n : names[k].len);
                pattern[i] = pattern[k];
                str_assign(t, &names[i]);
            }
            break;
        case 3:                        /* a$ = "" */
            str_release(&names[i]);
            names[i].len = 0;
            break;
        case 5: {                      /* PAINT: all the free room as one temporary, then freed */
            uword room = str_free_bytes();
            char *data;
            SD *held = str_tmp(room, &data);
            memset(data, 0x5A, room);
            check(step);
            str_tmp_free(held);
            break;
        }
        case 6: {                      /* the heap takes the free end of string space, and gives it back */
            uword room = str_give_tail();
            if (room)
                str_take(room);
            break;
        }
        case 7:                        /* every string packed together */
            str_compact();
            break;
        case 4:                        /* a$ = b$ + c$ (two temporaries joined, then freed) */
            {
                SD *x = make(len, (unsigned char)(i * 3));
                SD *y = make(next() % 9, 1);
                unsigned total = x->len + y->len, j;
                char *data;
                SD *joined = str_tmp(total, &data);
                for (j = 0; j < x->len; j++) data[j] = x->ptr[j];
                for (j = 0; j < y->len; j++) data[x->len + j] = y->ptr[j];
                str_tmp_free(x);
                str_tmp_free(y);
                /* expected: x's bytes then y's; keep only strings whose bytes follow one run */
                str_tmp_free(joined);
            }
            break;
        }
        str_all_tmp_free(0);
        check(step);
    }
    puts("ok");
    return 0;
}
