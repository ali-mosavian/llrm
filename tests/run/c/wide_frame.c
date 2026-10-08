// flags: -O0 -march=i486 | -O2 -march=i486
// targets: x86-m16
// A local past 32 KB below bp. The displacement does not fit a signed word, and real mode adds it modulo 64 KB, so the encoding
// is the same address with the wrapped displacement (gcc.c-torture 921113-1: "_gitter: Semantics(op=FLOAT_STORE ...").
// At -O2 an index of 32798 into the 32,800-byte array was folded as the i16 -32738, below the array: the frame became
// `sub sp, 65542`, 6 bytes after the wrap, and a call's pushes overwrote the array's tail (#905).
extern void report(long value);

static void note(float d) { report((long)(d * 2.0f)); }

static void frame(float seed)
{
    float d;
    int big[16400];
    int i;
    long sum = 0;

    big[0] = 7;
    for (i = 16380; i < 16400; i++)
        big[i] = i - 16370;
    d = seed + big[0] + big[16399];
    note(d);
    for (i = 16380; i < 16400; i++)
        sum += big[i];
    report(sum);
}

int main(void)
{
    frame(1.5f);
    return 0;
}
