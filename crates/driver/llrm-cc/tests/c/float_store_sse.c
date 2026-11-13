extern void report(long value);

volatile long big = 16777217L;
volatile float single = 16777216.0f;
volatile double twin = 16777216.0;

void compare(void)
{
    report(big != single);
    report(big != twin);
}
