long next(void);
void give(long value);

long saved;

short grab(void)
{
    saved = next();
    if (saved != 0)
        return 1;
    return 0;
}

void pass(void)
{
    give(next());
}
