extern void use(short *);
short counts[256];

void clear(void)
{
    __builtin_memset(counts, 0, sizeof counts);
    use(counts);
}
