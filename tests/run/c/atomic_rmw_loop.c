// flags: -O1 | -O2 | -Os
// targets: m68k-68020
// A read-modify-write is a compare-exchange loop with `cas` (AtomicExpandPass); each result is the old value or the new memory.
extern void report(long value);

static int word = 7;
static unsigned char byte = 200;

#define OP(name, type, expr) __attribute__((noinline)) type name(type *p, type v) { return expr; }
OP(fetch_add, int, __atomic_fetch_add(p, v, __ATOMIC_SEQ_CST))
OP(fetch_sub, int, __atomic_fetch_sub(p, v, __ATOMIC_SEQ_CST))
OP(fetch_and, int, __atomic_fetch_and(p, v, __ATOMIC_SEQ_CST))
OP(fetch_or, int, __atomic_fetch_or(p, v, __ATOMIC_SEQ_CST))
OP(fetch_xor, int, __atomic_fetch_xor(p, v, __ATOMIC_SEQ_CST))
OP(fetch_nand, int, __atomic_fetch_nand(p, v, __ATOMIC_SEQ_CST))
OP(exchange, int, __atomic_exchange_n(p, v, __ATOMIC_SEQ_CST))
OP(fetch_max, int, __atomic_fetch_max(p, v, __ATOMIC_SEQ_CST))
OP(fetch_umin_byte, unsigned char, __atomic_fetch_min(p, v, __ATOMIC_ACQ_REL))

__attribute__((noinline)) int swap_if(int *p, int expected, int desired)
{
    return __atomic_compare_exchange_n(p, &expected, desired, 0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST) ? 100 + expected : expected;
}

int main(void)
{
    report(fetch_add(&word, 5));
    report(word);
    report(fetch_sub(&word, 20));
    report(word);
    report(fetch_and(&word, 0xff));
    report(word);
    report(fetch_or(&word, 0x100));
    report(fetch_xor(&word, 0x1ff));
    report(word);
    report(fetch_nand(&word, 0x0f));
    report(word);
    report(exchange(&word, -3));
    report(fetch_max(&word, 9));
    report(word);
    report(fetch_umin_byte(&byte, 150));
    report(byte);
    report(swap_if(&word, 9, 11));
    report(word);
    report(swap_if(&word, 9, 12));
    report(word);
    return 0;
}
