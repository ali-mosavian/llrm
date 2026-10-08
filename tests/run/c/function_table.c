// flags: -O0 | -O2 | -Os
// A table naming a function the unit only declares, called only through the table (gcc.c-torture 921110-1: `frob f[] =
// {abort}`): nothing calls `report` by name, so nothing declared it external and the object writer refused it
// ("references to nothing defined or declared").
extern void report(long value);

typedef void (*sink)(long);

sink table[2] = { report, report };

int main(void)
{
    int i;
    for (i = 0; i < 2; ++i)
        table[i](7 + i);
    return 0;
}
