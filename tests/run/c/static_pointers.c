// flags: -O2 -march=i486 | -Os -march=i486 | -O2 -march=i486 -m32 | -Os -march=i486 -m32
/* Pointers in static initializers: a table of functions, a function, a pointer into an array and a string.
   Under -m32 a near pointer is a dword; they were emitted as words (`dw _add`), so calls through them
   jumped to the low half of the address and the data pointers read the wrong cells. */
extern void report(long value);
static int add(int a, int b) { return a + b; }
static int mul(int a, int b) { return a * b; }
int (*ops[2])(int, int) = { add, mul };
int (*one)(int, int) = mul;
int cells[4] = { 10, 20, 30, 40 };
int *inner = &cells[2];
char *name = "ab";

static int pick(int i, int a)
{
    switch (i) {
    case 0: return a + 1;
    case 1: return a + 2;
    case 2: return a * 3;
    case 3: return a - 4;
    case 4: return a << 2;
    case 5: return a ^ 7;
    default: return -1;
    }
}

int main(void)
{
    int (*local)(int, int) = ops[1];
    int i;
    long sum = 0;
    report(ops[0](3, 4));
    report(ops[1](3, 4));
    report(one(5, 6));
    report(*inner);
    report(name[1]);
    report(local(7, 8));
    for (i = 0; i < 2; i++)
        sum += ops[i](i + 2, 5);
    report(sum);
    for (i = 0; i < 7; i++)
        report(pick(i, 10));
    return 0;
}
