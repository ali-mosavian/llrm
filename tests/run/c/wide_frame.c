// flags: -O0 -march=i486
// targets: x86-m16
// A local past 32 KB below bp: the displacement does not fit a signed word, and real mode adds it modulo 64 KB, so the
// encoding is the same address with the wrapped displacement (gcc.c-torture 921113-1: "_gitter: Semantics(op=FLOAT_STORE ...").
extern void report(long value);

static void note(float d) { report((long)(d * 2.0f)); }

static void frame(float seed)
{
    float d;
    int big[16400];

    big[0] = 7;
    big[16399] = 9;
    d = seed + big[0] + big[16399];
    note(d);
}

int main(void)
{
    frame(1.5f);
    return 0;
}
