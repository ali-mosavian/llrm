struct pt { int x; int y; };

int counter = 7;

int add(int a, int b)
{
    int sum = a + b;
    return sum + counter;
}

int main(void)
{
    struct pt p;
    p.x = 3;
    p.y = add(p.x, 4);
    return p.y;
}
