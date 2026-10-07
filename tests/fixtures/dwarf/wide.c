long long total;

long long sum(long long a, int b)
{
    long long t = a + b;
    unsigned long long u = 7;
    return t + total + u;
}

int main(void)
{
    total = 4294967297LL;
    return (int)sum(5000000000LL, 3);
}
