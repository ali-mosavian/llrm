int g(int *);

int f(int a)
{
    int cell[4];
    cell[0] = a;
    return g(cell) + a;
}
