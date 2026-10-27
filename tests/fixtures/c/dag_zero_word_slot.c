// A word local zeroed in a loop: --isel=dag stored a dword zero into its 2-byte spill slot and zeroed the neighbour (bench/grep: n, 10x the instructions).
extern int input_read(char *buffer, int count);
static unsigned char nxt[80 * 32], classof[256], hit[80];

long scan(long *checksum)
{
    char buffer[1024];
    int s = 0, n, i, inline_hit = 0;
    long lines = 0, sum = 0, pos = 0;

    while ((n = input_read(buffer, 1024)) > 0) {
        for (i = 0; i < n; ++i) {
            unsigned char b = (unsigned char)buffer[i];
            s = nxt[s * 32 + classof[b]];
            if (hit[s]) {
                inline_hit = 1;
                sum = ((sum << 1) + pos + hit[s]) & 0x1FFFFFFFL;
            }
            if (b == 10) {
                lines += inline_hit;
                inline_hit = 0;
            }
            ++pos;
        }
    }
    *checksum = sum;
    return lines + inline_hit;
}
