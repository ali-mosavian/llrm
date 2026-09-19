/* Dynamic far-struct fields from qc-port's cache-list update. */
typedef struct {
    short far *bord, far *bprev, far *bnext;
    long far *bstamp;
    long clock;
    short lhead[7], ltail[7];
} Cache;

static short bord[3], bprev[3], bnext[3];
static long bstamp[3];
static Cache cache;

static void lruUse(Cache far *sc, short block)
{
    short chain, tail;

    if (block < 0) return;
    sc->bstamp[block] = ++sc->clock;
    chain = sc->bord[block];
    if (sc->bprev[block] >= 0 || sc->bnext[block] >= 0 || sc->lhead[chain] == block) return;
    tail = sc->ltail[chain];
    sc->bprev[block] = tail;
    sc->bnext[block] = -1;
    if (tail >= 0) sc->bnext[tail] = block; else sc->lhead[chain] = block;
    sc->ltail[chain] = block;
}

long bench_lru(short block)
{
    short index;

    cache.bord = bord;
    cache.bprev = bprev;
    cache.bnext = bnext;
    cache.bstamp = bstamp;
    cache.clock = 7;
    for (index = 0; index < 3; ++index) {
        bord[index] = 0;
        bprev[index] = -1;
        bnext[index] = -1;
        bstamp[index] = 0;
    }
    for (index = 0; index < 7; ++index) {
        cache.lhead[index] = -1;
        cache.ltail[index] = -1;
    }
    bord[1] = 2;
    cache.lhead[2] = 0;
    cache.ltail[2] = 0;

    lruUse(&cache, block);
    return cache.clock + bstamp[block] * 3L + bprev[block] * 5L
        + (bnext[block] + 1) * 7L + bnext[0] * 11L
        + cache.lhead[2] * 13L + cache.ltail[2] * 17L;
}
