typedef struct {
    unsigned short a, b;
    unsigned c;
} Desc;

extern unsigned finish(unsigned c, unsigned id);

/* Defined here and not inlined, so the pointer is known not to be captured
   and the last call is marked `tail`. */
__attribute__((noinline)) void get(Desc *out, unsigned id)
{
    out->a = (unsigned short)id;
    out->b = (unsigned short)(id >> 16);
    out->c = id * 3;
}

unsigned local_then_tail(unsigned id)
{
    Desc d;
    get(&d, id);
    return finish(d.c, id);
}
