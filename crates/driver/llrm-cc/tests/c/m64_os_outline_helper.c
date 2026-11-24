extern unsigned char *arena;
typedef struct { unsigned names, text; unsigned short kind; } Syms;

static inline unsigned char *name_at(const Syms *s, unsigned id)
{
    return arena + s->names + id * 28u + s->kind;
}

void tag(const Syms *s, unsigned a, unsigned b, unsigned c, unsigned d)
{
    name_at(s, a)[4] = 1;
    name_at(s, b)[8] = 2;
    name_at(s, c)[12] = 3;
    name_at(s, d)[16] = 4;
}

void untag(const Syms *s, unsigned a, unsigned b, unsigned c)
{
    name_at(s, a)[4] = 0;
    name_at(s, b)[8] = 0;
    name_at(s, c)[12] = 0;
}
