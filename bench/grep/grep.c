// flags: -O2 -march=i486 | -O2 -march=i486 -m32
// data: @dickens
/* Aho-Corasick over the bytes of DICKENS: the automaton is built once from ten patterns, then a table walk
   scans the file in 1K chunks. Prints the lines holding a match and a checksum of the match positions. */
extern void report(long value);
extern int input_read(char *buffer, int count);

#define STRIDE 32 /* a state's row: its classes are fewer */

static const char patterns[] = "Oliver|Scrooge|Copperfield|Dombey|Marley|Micawber|Heep|ghost|gentleman|Tiny Tim|";
static unsigned char nxt[80 * STRIDE], classof[256], fail[80], hit[80], queue[80];

long bench_grep(long *checksum)
{
    char buffer[1024];
    const char *p;
    int classes = 1, states = 1, id = 1, s = 0, head = 0, tail = 0, cl, n, i, inline_hit = 0;
    long lines = 0, sum = 0, pos = 0;

    for (p = patterns; *p; ++p) {
        unsigned char c = (unsigned char)*p;
        if (c == '|') {
            hit[s] = (unsigned char)id++;
            s = 0;
            continue;
        }
        if (!classof[c]) classof[c] = (unsigned char)classes++;
        if (!nxt[s * STRIDE + classof[c]]) nxt[s * STRIDE + classof[c]] = (unsigned char)states++;
        s = nxt[s * STRIDE + classof[c]];
    }
    for (cl = 1; cl < classes; ++cl)
        if (nxt[cl]) queue[tail++] = nxt[cl];
    while (head < tail) {
        s = queue[head++];
        for (cl = 0; cl < classes; ++cl) {
            int t = nxt[s * STRIDE + cl], f = nxt[fail[s] * STRIDE + cl];
            if (t) {
                fail[t] = (unsigned char)f;
                if (!hit[t]) hit[t] = hit[f];
                queue[tail++] = (unsigned char)t;
            } else
                nxt[s * STRIDE + cl] = (unsigned char)f;
        }
    }
    s = 0;
    while ((n = input_read(buffer, 1024)) > 0) {
        for (i = 0; i < n; ++i) {
            unsigned char b = (unsigned char)buffer[i];
            s = nxt[s * STRIDE + classof[b]];
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

int main(void)
{
    long sum;
    long lines = bench_grep(&sum);

    report(lines);
    report(sum);
    return 0;
}
