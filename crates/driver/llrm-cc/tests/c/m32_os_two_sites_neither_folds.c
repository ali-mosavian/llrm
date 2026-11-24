typedef unsigned u32;
typedef unsigned char u8;
extern void *pmem_resize(void *, unsigned long);
extern void *memset(void *, int, unsigned long);
extern void *memcpy(void *, const void *, unsigned long);
extern void fault(int) __attribute__((noreturn));

extern u8 *carve(void *, u32);
extern void ast_init(void *, void *);
typedef struct { u8 *buf; u32 cap; int full; u8 *cbuf; void *allo, *ast, *flt; u32 dfa; } Comp;

static inline _Bool grow_room(void *const items, u32 *const cap, const u32 n, const u32 size)
{
    if (n <= *cap)
        return 1;
    const u32 half = *cap + *cap / 2;
    u32 want = half > n ? half : n;
    want = want < 64 ? 64 : want;
    void *old;
    memcpy(&old, items, sizeof old);
    u8 *const p = pmem_resize(old, (unsigned long)want * size);
    if (!p)
        return 0;
    memset(p + (unsigned long)*cap * size, 0, (unsigned long)(want - *cap) * size);
    memcpy(items, &p, sizeof p);
    *cap = want;
    return 1;
}

void init_a(Comp *const c)
{
    c->cbuf = carve(c->allo, 1 + 255 + 64) + 1;
    if (!grow_room(&c->buf, &c->cap, 4096, 1))
        fault(1);
    c->dfa = 0xFFFF;
    ast_init(c->ast, c->flt);
}

void init_b(Comp *const c)
{
    c->cbuf = carve(c->allo, 1 + 127 + 32) + 1;
    if (!grow_room(&c->buf, &c->cap, 8192, 1))
        fault(2);
    c->dfa = 0x7FFF;
    ast_init(c->flt, c->ast);
}
