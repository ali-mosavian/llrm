// flags: -O0 | -O2
// An integer constant compared with or added to a float constant converts to the float's type first (C11 6.3.1.8), so
// 16777217L is 16777216.0f. Open Watcom's constant folder made it a double and folded `16777217L != 16777216.0f` to 1
// (gcc.c-torture 920710-1, reduced).
extern void report(long value);

int main(void)
{
    report(16777217L != 16777216.0f);
    report(16777217L == (float)16777217e0);
    report(16777217L != 16777216.0);
    report(16777216.0f < 16777217L);
    report(16777217L + 0.0f == 16777216.0f);
    report(16777217L + 0.0 == 16777216.0);
    return 0;
}
