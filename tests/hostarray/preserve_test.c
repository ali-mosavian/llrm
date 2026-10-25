/* REDIM PRESERVE (array.c, m32 ad.c) on the host: growing keeps the old elements and zeroes the new, shrinking
   truncates, and a change of a dimension but the outermost is Subscript out of range. Exits 0, or 1 with what broke. */
#include <stdio.h>
#include <stdlib.h>
#include <setjmp.h>
#include "../../runtime/qb/array.c"
#include "../../runtime/qb/dos/m32/ad.c"

static jmp_buf failed;
static unsigned raised;
void qb_error(unsigned n) { raised = n; longjmp(failed, 1); }
void str_release(SD *sd) { (void)sd; }
/* block.asm's B$COPY on the host: a forward byte copy. */
void copy_bytes(char *to, const char *from, unsigned n) { while (n--) *to++ = *from++; }
void lh_on_move(enum LhType type, LhMoved moved) { (void)type; (void)moved; }
char *heap_low, *heap_top;
void *lh_alloc(uword bytes, enum LhType type, void *owner, byte file)
{
    LhEntry *entry = calloc(1, sizeof *entry + bytes);

    entry->type = type; entry->file = file; entry->owner = owner;
    return entry + 1;
}
void lh_free(void *data) { free((LhEntry *)data - 1); }
LhEntry *lh_entry(void *data) { return (LhEntry *)data - 1; }
unsigned long str_free_bytes(void) { return 0; }

typedef struct { DimCall call; int more[2]; } Call;

static void dim(enum DimMode mode, AD *ad, int rank, int lower0, int upper0, int upper1)
{
    Call c = { { ad, rank | (0x01 << 8), 2, { { upper0, lower0 } } }, { 0, 0 } };

    if (rank == 2) { c.call.bounds[0].upper = upper1; c.call.bounds[0].lower = 0; ((int *)&c.call.bounds[1])[0] = upper0; ((int *)&c.call.bounds[1])[1] = lower0; }
    array_dim(mode, &c.call);
}

static int fail(const char *what) { printf("%s\n", what); return 1; }

int main(void)
{
    static char storage[sizeof(AD) + 4 * sizeof(DM)];
    AD *ad = (AD *)storage;
    short *e;
    int i;

    if (setjmp(failed)) return fail("an unexpected error");
    dim(DIM_ALLOCATE, ad, 1, 0, 5, 0);
    e = ad->data;
    for (i = 0; i <= 5; i++) e[i] = 100 + i;

    dim(DIM_PRESERVE, ad, 1, 0, 10, 0);   /* grow */
    e = ad->data;
    for (i = 0; i <= 5; i++) if (e[i] != 100 + i) return fail("grow lost an old element");
    for (i = 6; i <= 10; i++) if (e[i] != 0) return fail("grow left a new element nonzero");
    if (ad->dm[0].count != 11) return fail("grow did not describe the new bound");

    dim(DIM_PRESERVE, ad, 1, 0, 2, 0);    /* shrink */
    e = ad->data;
    for (i = 0; i <= 2; i++) if (e[i] != 100 + i) return fail("shrink lost a kept element");
    if (ad->dm[0].count != 3) return fail("shrink did not describe the new bound");

    if (setjmp(failed) == 0) { dim(DIM_PRESERVE, ad, 1, 1, 4, 0); return fail("a changed lower bound was accepted"); }
    if (raised != BE_SUBSCRIP) return fail("a changed lower bound raised the wrong error");

    dim(DIM_REALLOCATE, ad, 1, 0, 3, 0);  /* plain REDIM clears */
    e = ad->data;
    for (i = 0; i <= 3; i++) if (e[i] != 0) return fail("REDIM kept an element");
    printf("ok\n");
    return 0;
}
