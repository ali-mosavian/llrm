/* input_read for bench/grep without the Silesia download: a deterministic ~1.5 MB pseudo-text with the
   benchmark's patterns among filler words. Same bytes for every compiler; restarts after end of input. */
#include <string.h>
#define TOTAL 1500000
static unsigned char buf[TOTAL]; static long made, pos;
static const char *words[] = {"the","and","Oliver","of","Scrooge","to","a","in","ghost","was","Copperfield","he","that","Dombey","it","his","gentleman","Marley","Micawber","Heep","Tiny Tim","said","with","for","had","not","but"};
static void make(void) {
    unsigned s = 12345; long n = 0; made = 1;
    while (n < TOTAL - 16) {
        s = s * 1103515245u + 12345u; const char *w = words[(s >> 16) % 27]; long l = strlen(w);
        memcpy(buf + n, w, l); n += l; s = s * 1103515245u + 12345u;
        buf[n++] = ((s >> 16) % 9 == 0) ? '\n' : ' ';
    }
    memset(buf + n, ' ', TOTAL - n);
}
int input_read(char *b, int count) {
    if (!made) make();
    if (pos >= TOTAL) { pos = 0; return 0; }
    long n = TOTAL - pos < count ? TOTAL - pos : count; memcpy(b, buf + pos, n); pos += n; return (int)n;
}
