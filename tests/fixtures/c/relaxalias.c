/* -oa: what this unit never takes the address of, no pointer reaches. */
extern int counter;
int seen;

int after_store(int *p)
{
    int a = counter + seen;
    *p = 5;
    return a + counter + seen;
}
