volatile int sink;

int leaf(int a, int b, int c, int d, int e, int f)
{
    sink = a + f;
    return a + b + c + d + e + f;
}

/* No variable of its own: no frame register, so its stack pointer moves with each argument pushed. */
int middle(void)
{
    return leaf(1, 2, 3, 4, 5, 6) + leaf(2, 3, 4, 5, 6, 7);
}

int main(void)
{
    return middle();
}
