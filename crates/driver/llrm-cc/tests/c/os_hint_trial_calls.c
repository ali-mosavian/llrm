/* A hinted callee with 15 straight-line sites: kept as calls the object is a third
   of the inlined one (m32 -Os .text 209 B against 637 B); gcc keeps the calls. */
extern unsigned char buf[];
extern unsigned n;
void overflow(void);

static inline void put(unsigned char b)
{
    if (n >= 4096)
        overflow();
    buf[n] = b;
    n++;
    buf[n] = 0;
}

void f(void)
{
    put(1); put(2); put(3); put(4); put(5); put(6); put(7); put(8);
}

void g(void)
{
    put(9); put(10); put(11); put(12); put(13); put(14); put(15);
}
