static unsigned short counts[64];

void bump(unsigned short limit, unsigned short i)
{
    unsigned short j;
    for (j = i * i; j < limit; j += i)
        counts[j & 63]++;
}

int main(void)
{
    bump(500, 7);
    return counts[5];
}
