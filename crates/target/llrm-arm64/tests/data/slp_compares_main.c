#include <stdio.h>
#include <string.h>

static const char *line;
static int calls;

long env(int index, char *buffer, int size)
{
    (void)index;
    (void)size;
    calls++;
    memset(buffer, 0, 16);
    memcpy(buffer, line, strlen(line));
    return (long)strlen(line);
}

int comspec(void);
int four(void);
int two(void);
int under_or(void);

int main(void)
{
    const char *cases[] = {"COMSPEC=x", "COMSPEC=", "COMSPEC=abc", "XOMSPEC=x", "COMSPEX=x", "COMSPDC=x",
                           "ABCD", "ABCE", "XBCD", "AB", "AX", "CD", "ABxx", "xxCD", "ABCDxxxxxxxxxxx"};
    int bad = 0;
    for (unsigned i = 0; i < sizeof cases / sizeof *cases; i++) {
        const char *c = line = cases[i];
        long n = (long)strlen(c);
        int want_comspec = n > 8 && !strncmp(c, "COMSPEC=", 8);
        int want_four = !strncmp(c, "ABCD", 4);
        int want_two = !strncmp(c, "AB", 2);
        int want_or = !strncmp(c, "AB", 2) || (c[2] == 'C' && c[3] == 'D');
        if (comspec() != want_comspec || four() != want_four || two() != want_two || under_or() != want_or) {
            printf("wrong for \"%s\"\n", c);
            bad = 1;
        }
    }
    return bad;
}
