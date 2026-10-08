int id(int x)
{
    return x + 1;
}

int work(int a)
{
    int k = a * 3;
    int r = id(k);
    return r + k;
}

int main(void)
{
    return work(4) + work(5);
}
