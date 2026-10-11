// flags: -O2 | -O2 -mabi=watcom | -O2 -mabi=cdecl | -O2 -m32 | -O2 -m32 -mabi=sysv
/* File externs: write 1000 bytes in two calls, read them back in chunks of 300 (the last short, then 0 at the end), a missing file is below
   zero. Without them C had no way to write a file by name. */
extern void report(long value);
extern int file_open(const char *name, int mode);
extern int file_read(int handle, char *buffer, int count);
extern int file_write(int handle, const char *buffer, int count);
extern int file_close(int handle);

static char data[500], back[300];

int main(void)
{
    int i, h, n;
    unsigned long sum = 0;

    for (i = 0; i < 500; ++i)
        data[i] = (char)(i * 7 + 1);
    h = file_open("FILEIO.TMP", 1);
    report(h >= 0);
    report(file_write(h, data, 500));
    report(file_write(h, data, 500));
    report(file_close(h));
    h = file_open("FILEIO.TMP", 0);
    while ((n = file_read(h, back, 300)) > 0) {
        report(n);
        for (i = 0; i < n; ++i)
            sum = sum * 31 + (unsigned char)back[i];
    }
    report(n);
    report((long)sum);
    report(file_close(h));
    report(file_open("NOSUCH.FIL", 0) < 0);
    return 0;
}
