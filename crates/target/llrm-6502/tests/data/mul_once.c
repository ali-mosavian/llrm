extern void report(long value);

static volatile long seed = 12345L;

int main(void)
{
    long classes = seed;
    classes = classes * 531441L + 250;
    report(classes);
    return 0;
}
