static volatile unsigned char junk[96];

void fill(void)
{
    int i;
    for (i = 0; i < 96; ++i)
        junk[i] = 0xFF;
}

int main(void)
{
    fill();
    return junk[5];
}
