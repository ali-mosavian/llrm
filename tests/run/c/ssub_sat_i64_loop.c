// flags: -O1 | -O2 | -Os
// bar: none (SDCC error 101: too many parameters)
// A saturating i64 subtract in a loop: the select's sunk arm (xor of the saturated word) cleared the flags the next
// select read, so the overflow row gave the wrapped 0 for the saturated -1 (word 0 of q[0] - q[1]).
extern void report(long value);

typedef long long i64;

#define CASE(T, OP, A, B) \
    do { T r = __builtin_elementwise_##OP##_sat(A, B); report((long)r); report((long)(r / 256)); } while (0)

i64 qa[3] = {0x7000000000000000LL, -0x7000000000000000LL, 3000000000LL};

int main(void)
{
    volatile i64 *q = qa;
    int i, j;
    for (i = 0; i < 1; i++)
        for (j = 0; j < 3; j++) {
            CASE(i64, sub, q[i], q[j]);
        }
    return 0;
}
