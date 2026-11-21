int printf(const char *, ...);

enum { PUSH, ADD, MUL, DUP, JNZ, DEC, HALT };

__attribute__((noinline)) long run(const int *code)
{
    static const void *const table[] = {&&do_push, &&do_add, &&do_mul, &&do_dup, &&do_jnz, &&do_dec, &&do_halt};
    long stack[16];
    long *sp = stack;
    const int *pc = code;
#define NEXT goto *table[*pc++]
    NEXT;
do_push:
    *sp++ = *pc++;
    NEXT;
do_add:
    sp--;
    sp[-1] += sp[0];
    NEXT;
do_mul:
    sp--;
    sp[-1] *= sp[0];
    NEXT;
do_dup:
    sp[0] = sp[-1];
    sp++;
    NEXT;
do_dec:
    sp[-1] -= 1;
    NEXT;
do_jnz:
    pc = sp[-1] ? code + *pc : pc + 1;
    NEXT;
do_halt:
    return sp[-1];
}

__attribute__((noinline)) int jump_to(int which, int x)
{
    void *target = which ? &&one : &&two;
    goto *target;
one:
    return x + 1;
two:
    return x * 2;
}

int main(void)
{
    const int program[] = {PUSH, 3, PUSH, 4, ADD, PUSH, 6, DEC, JNZ, 7, HALT};
    const int second[] = {PUSH, 5, DUP, MUL, PUSH, 2, ADD, HALT};
    printf("%ld %ld %d %d\n", run(program), run(second), jump_to(1, 10), jump_to(0, 10));
    return 0;
}
