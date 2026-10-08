extern void report(long value);

static unsigned table[256];
long bench_x_crc32(int n)
{
    unsigned c, crc = 0xFFFFFFFFu; int i, k;
    for (i = 0; i < 256; ++i) { c = (unsigned)i; for (k = 0; k < 8; ++k) c = (c & 1) ? 0xEDB88320u ^ (c >> 1) : c >> 1; table[i] = c; }
    for (i = 0; i < n; ++i) crc = table[(crc ^ (unsigned)(i * 7 + (i >> 3))) & 255] ^ (crc >> 8);
    return (long)(crc & 0x7FFFFFFF);
}

int main(void)
{
    report(bench_x_crc32(2000));
    return 0;
}
