// flags: -O0 | -O2 | -Os | -O2 -fno-unroll-loops -fno-peel-loops
// A switch whose cases are empty and leave to the code after it, which is also its default (gcc.c-torture 920506-1):
// at -O0 the empty blocks are threaded away and a conditional branch had the one block for both its edges, which the
// printer refused ("block N leaves for (5, 5) with no instruction choosing").
extern void report(long value);

static int seen;

static void bump(int by) { seen = seen * 3 + by; }

static void classify(int value)
{
    switch (value) {
    case 1:
        break;
    case 2:
        break;
    case 3:
    case 4:
        break;
    }
    bump(value + 1);
}

int main(void)
{
    int i;
    for (i = 0; i < 6; ++i)
        classify(i);
    report(seen);
    return 0;
}
