/* An empty kernel: the whole-program time of this is each toolchain's start-up and one print. */
extern void report(long value);

int main(void)
{
    report(0);
    return 0;
}
