/* A declaration list shares one type specifier: a pointer in it must not qualify its neighbours. */
char buf[4], *p, **pp, c;
static const char *names[2], *last;
struct R { char *command, *label; int k, *more; };
unsigned s_buf = sizeof(buf), s_c = sizeof(c), s_R = sizeof(struct R);
int use(int n)
{
    char text[8], *t = text, *u;
    int i, *ip = &i;
    u = t;
    return n + (int)(u - text) + (ip != 0);
}
