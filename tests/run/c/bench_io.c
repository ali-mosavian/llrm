// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* bench/bench_io.h under llrm: write a file by name, read it back in two chunks, a missing file is below zero. */
#include "../../../bench/bench_io.h"
extern void report(long value);

static unsigned char data[300], back[200];

int main(void)
{
    int i, handle, n;

    for (i = 0; i < 300; ++i)
        data[i] = (unsigned char)(i * 3);
    handle = bench_create("BENCHIO.TMP");
    report(bench_write(handle, data, 300));
    report(bench_close(handle));
    handle = bench_open_read("BENCHIO.TMP");
    while ((n = bench_read(handle, back, 200)) > 0)
        report(n * 1000L + back[n - 1]);
    report(n);
    bench_close(handle);
    report(bench_open_read("NOSUCH.FIL") < 0);
    return 0;
}
