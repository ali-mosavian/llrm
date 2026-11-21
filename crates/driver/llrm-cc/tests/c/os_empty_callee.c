/* A callee whose only effect is a store to a global that nothing reads once `main` has inlined
 * the chain: the store goes, the callee is empty, and the call that forwards to it goes too. */
extern void report(long value);

static int total;

void add_row(const int *row, int bias)
{
    total = row[0] * 3 + row[1] * 5 + row[2] * 7 + row[3] * 11 + row[4] * 13 + bias;
}

void forward(const int *row)
{
    add_row(row, 6);
}

int main(void)
{
    int row[5] = {1, 2, 3, 4, 5};

    forward(row);
    report(total);
    return 0;
}
