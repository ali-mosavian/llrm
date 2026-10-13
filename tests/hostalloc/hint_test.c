/* Free string entries joined by a scan must not leave the allocation hint inside the result.  The heap then gave string
   space more room, str_take grew the stale entry instead of the joined one, and the entries stopped tiling the space:
   jpeg_single at -m32 -O1 looped forever in the heap walk.  Exits 0, or 1 with what broke. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../../runtime/qb/nhstutil.c"
#include "../../runtime/qb/stcore.c"

void qb_error(unsigned n) { fprintf(stderr, "qb_error %u\n", n); exit(3); }
void copy_bytes(char *to, const char *from, unsigned n) { while (n--) *to++ = *from++; }
int lh_take_from_strings(void) { return 0; }
void *lh_alloc(uword b, enum LhType t, void *o, byte f) { (void)b; (void)t; (void)o; (void)f; return 0; }
void lh_free(void *d) { (void)d; }

enum { POOL = 4096, SPACE = 1024 };
static char pool[POOL];

/* The heap's free room at the boundary: the second half of the pool, once. */
void lh_give_free_to_strings(void)
{
    static int given;

    if (!given++)
        str_take(POOL - SPACE);
}

int main(void)
{
    SD a, b, big;
    StrEntry *entry;
    char *end;

    memset(pool, 0xAA, sizeof pool);
    str_init(pool, pool + SPACE);
    str_alloc(&a, 300);
    str_alloc(&b, 300);
    str_release(&a);
    str_release(&b);
    /* The free tail is the hint; the scan from the start joins a, b and it. */
    str_alloc(&big, SPACE + 100);
    end = pool + POOL;
    for (entry = str_first; !AT_END(entry); entry = FOLLOWING(entry))
        if ((char *)entry < pool || (char *)FOLLOWING(entry) > end) {
            puts("the entries do not tile the space");
            return 1;
        }
    if ((char *)entry != str_end) {
        puts("the entries do not end at the boundary");
        return 1;
    }
    puts("ok");
    return 0;
}
