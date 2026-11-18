// flags: -O2 -march=i486 | -O2 -march=i486 -m32
// data: @dickens
/* Twin of grep.nib with its semantics: every index checked against the array length, the read buffer zero-initialised. */
/* Aho-Corasick over the bytes of DICKENS: the automaton is built once from ten patterns, then a table walk
   scans the file in 1K chunks. Prints the lines holding a match and a checksum of the match positions. */
extern void report(long value);
extern int input_read(char *buffer, int count);
extern void ebnd(void);
#define CK(i, n) ((unsigned)(i) < (n) ? (i) : (ebnd(), 0))

#define STRIDE 32 /* a state's row: its classes are fewer */

static const char patterns[] = "Oliver|Scrooge|Copperfield|Dombey|Marley|Micawber|Heep|ghost|gentleman|Tiny Tim|";
static unsigned char nxt[80 * STRIDE], classof[256], fail[80], hit[80], queue[80];

long bench_grep(long *checksum)
{
    char buffer[1024] = {0};
    const char *p;
    int classes = 1, states = 1, id = 1, s = 0, head = 0, tail = 0, cl, n, i, inline_hit = 0;
    long lines = 0, sum = 0, pos = 0;

    for (p = patterns; *p; ++p) {
        unsigned char c = (unsigned char)*p;
        if (c == '|') {
            hit[CK(s, 80)] = (unsigned char)id;
            ++id;
            s = 0;
            continue;
        }
        if (!classof[CK(c, 256)]) classof[CK(c, 256)] = (unsigned char)classes++;
        if (!nxt[CK(s * STRIDE + classof[CK(c, 256)], 80 * STRIDE)]) nxt[CK(s * STRIDE + classof[CK(c, 256)], 80 * STRIDE)] = (unsigned char)states++;
        s = nxt[CK(s * STRIDE + classof[CK(c, 256)], 80 * STRIDE)];
    }
    for (cl = 1; cl < classes; ++cl)
        if (nxt[CK(cl, 80 * STRIDE)]) {
            queue[CK(tail, 80)] = nxt[CK(cl, 80 * STRIDE)];
            ++tail;
        }
    while (head < tail) {
        s = queue[CK(head, 80)];
        ++head;
        for (cl = 0; cl < classes; ++cl) {
            int t = nxt[CK(s * STRIDE + cl, 80 * STRIDE)], f = nxt[CK(fail[CK(s, 80)] * STRIDE + cl, 80 * STRIDE)];
            if (t) {
                fail[CK(t, 80)] = (unsigned char)f;
                if (!hit[CK(t, 80)]) hit[CK(t, 80)] = hit[CK(f, 80)];
                queue[CK(tail, 80)] = (unsigned char)t;
                ++tail;
            } else
                nxt[CK(s * STRIDE + cl, 80 * STRIDE)] = (unsigned char)f;
        }
    }
    s = 0;
    while ((n = input_read(buffer, 1024)) > 0) {
        for (i = 0; i < n; ++i) {
            unsigned char b = (unsigned char)buffer[CK(i, 1024)];
            s = nxt[CK(s * STRIDE + classof[b], 80 * STRIDE)];
            if (hit[CK(s, 80)]) {
                inline_hit = 1;
                sum = ((sum << 1) + pos + hit[CK(s, 80)]) & 0x1FFFFFFFL;
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

void ebnd(void) {}

int main(void)
{
    long sum;
    long lines = bench_grep(&sum);

    report(lines);
    report(sum);
    return 0;
}
