/* A closed private recursion SCC cannot return to its caller. */
static void spinSecond(void);

static void spinFirst(void)
{
    spinSecond();
}

static void spinSecond(void)
{
    spinFirst();
}

short entersRecursiveSpin(void)
{
    spinFirst();
    return 9;
}
