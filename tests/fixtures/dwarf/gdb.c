struct pt { short x; long y; };
struct pt gp;
int ga[10];

int add(int a, struct pt *p)
{
    int l = a + p->x;
    return l * 2 + (int)p->y;
}

int main(void)
{
    gp.x = 3;
    gp.y = 4;
    return add(1, &gp);
}
