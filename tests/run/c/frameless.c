// flags: -O2 -fno-inline-functions | -O2 -fno-inline-functions -m32
// A function that needs no frame register addresses its arguments and locals through the stack pointer,
// whatever has been pushed since the entry: a local array under four arguments, a call that pops its
// arguments, a local whose address is passed on, a local array walked by an index (ring read bytes
// above its frame), a recursion. Locals once shared the bytes of the arguments the next call pushed
// (queens printed 0, not 40).
extern void report(long value);

static long add4(long a, long b, long c, long d)
{
    return a + b * 2 + c * 3 + d * 4;
}

static long mix(long a, long b, long c)
{
    long t[4];

    t[0] = a;
    t[1] = b;
    t[2] = c;
    t[3] = add4(a, b, c, a + b);
    return t[0] * 3 + t[1] * 5 + t[2] * 7 + t[3];
}

static long chain(long n)
{
    long local[12], s = 0;
    int i;

    for (i = 0; i < 12; ++i) local[i] = n + i;
    for (i = 0; i < 12; ++i) s += add4(local[i], local[11 - i], n, i);
    return s;
}

static long ring(long n)
{
    long buf[64], total = 0;
    int i;

    for (i = 0; i < 64; ++i) buf[i] = (long)(i + 1) * 3 - 7 + n;
    for (i = 0; i < 300; ++i) total += buf[(i * 5 + 3) & 63];
    return total;
}

static void fill(long *p, int n)
{
    int i;

    for (i = 0; i < n; ++i) p[i] = i * i + 1;
}

static long address_taken(long n)
{
    long arr[8], s = 0;
    int i;

    fill(arr, 8);
    for (i = 0; i < 8; ++i) s += arr[i] * n;
    return s + mix(n, 1, 2);
}

static long descend(long n, long *total)
{
    long cell = n * 2;

    if (n == 0) return *total;
    *total += cell;
    return n - descend(n - 1, total) + cell;
}

int main(void)
{
    long total = 0;

    report(mix(1, 2, 3));
    report(chain(5));
    report(ring(2));
    report(address_taken(3));
    report(descend(10, &total));
    report(total);
    return 0;
}
