/* A short carried round a loop and sign-extended to index with: extended on its load, not again in every trip. */
long descent(const short *table)
{
    short at = 1;
    long steps = 0;
    do {
        at = table[at];
        ++steps;
    } while (at > 0);
    return steps;
}
