struct pt { int x; int y; };

int bump(struct pt *p, int by)
{
    p->x += by;
    return p->x;
}

int mix(int n)
{
    int a = 1;
    int b = 2;
    int c = 0;
    int i = 0;
    int v[4];
    struct pt p;
    struct pt q;
    v[0] = 0;
    v[1] = 0;
    v[2] = 0;
    v[3] = 0;
    p.x = n;
    p.y = n + 1;
    q.x = 0;
    q.y = 0;
    q = p;
    for (i = 0; i < 4; i++)
        v[i] = i * n;
    for (i = 0; i < 4; i++)
        c += v[i];
    if (c > 10)
        a = c;
    else
        b = c;
    b = b + a;
    c = bump(&p, b);
    do {
        a += 2;
        i--;
    } while (i > 0);
    return b + q.y + c + a;
}

int main(void)
{
    return mix(3);
}
