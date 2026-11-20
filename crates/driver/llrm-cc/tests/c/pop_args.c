extern int one(int);
extern int two(int, int);
int a(int x) { return one(x) + one(x + 1); }
int b(int x, int y) { return two(x, y) + two(y, x); }
int loop(int n)
{
    int s = 0;
    for (int i = 0; i < n; ++i) s += one(i);
    return s;
}
