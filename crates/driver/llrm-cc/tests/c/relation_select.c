/* A three-way relation as a byte: `a > b ? 1 : a == b ? 2 : 4`. */
unsigned char relation(int a, int b)
{
    return a > b ? 1 : a == b ? 2 : 4;
}
