typedef struct { short far *a; short far *b; } Pair;

void store_pair(Pair far *p, int i, int x, int y)
{
    p->a[i] = x;
    p->b[i] = y;
}
