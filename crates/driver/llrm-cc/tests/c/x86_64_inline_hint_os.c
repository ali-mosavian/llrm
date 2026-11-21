/* QB's operand handlers: the op runs on a local copy of the VM's registers, a
   switch on constants picks the operand and the operation. */
#include <stdint.h>

typedef struct { uint8_t *ip; int32_t *sp; uint32_t fp; } Regs;
typedef struct { Regs regs; uint8_t mem[256]; } Vm;
typedef enum { POPPED, VAR, LOC, U8, I16 } Src;
typedef enum { ADD, SUB, MUL, DIV, AND, OR, MOD, XOR } Op;

int fault(Vm *vm, int status);

static inline int32_t operand(Regs *const r, const Src src)
{
    switch (src) {
    case POPPED: return *r->sp++;
    case VAR: case LOC: {
        const uint32_t c = *(uint32_t *)r->ip;
        r->ip += 4;
        return *(int32_t *)((uint8_t *)r->sp + (src == LOC ? r->fp + c : c));
    }
    case U8: return *r->ip++;
    default: {
        const int16_t v = *(int16_t *)r->ip;
        r->ip += 2;
        return v;
    }
    }
}

static inline int divide(int32_t a, int32_t b, int32_t *q, int32_t *rem)
{
    if (!b) return 11;
    if (b == -1) {
        if (a == INT32_MIN) return 6;
        *q = -a;
        return 0;
    }
    *rem = a % b;
    *q = a / b;
    return 0;
}

static inline int hot_int(Regs *const r, const Src src, const Op op)
{
    const int32_t b = operand(r, src);
    int32_t *const top = r->sp;
    int32_t v = 0, rem = 0;
    int overflow = 0;

    switch (op) {
    case ADD: overflow = __builtin_add_overflow(*top, b, &v); break;
    case SUB: overflow = __builtin_sub_overflow(*top, b, &v); break;
    case MUL: overflow = __builtin_mul_overflow(*top, b, &v); break;
    case DIV: {
        const int st = divide(*top, b, &v, &rem);
        if (st) return st;
        break;
    }
    case AND: v = *top & b; break;
    case OR: v = *top | b; break;
    case MOD: {
        if (b != -1) {
            const int st = divide(*top, b, &v, &rem);
            if (st) return st;
        }
        v = rem;
        break;
    }
    case XOR: v = *top ^ b; break;
    }

    if (overflow) return 6;
    *top = v;
    return 0;
}

#define OPS(X, mode, src) \
    X(add##mode, src, ADD) X(sub##mode, src, SUB) X(mul##mode, src, MUL) \
    X(div##mode, src, DIV) X(and##mode, src, AND) X(or##mode, src, OR)
#define HANDLERS(X) \
    OPS(X, , POPPED) OPS(X, _v, VAR) OPS(X, _u8, U8) OPS(X, _i16, I16) OPS(X, _l, LOC)

#define X(name, src, op) \
    int oper_##name(Vm *vm) \
    { \
        Regs t = {vm->regs.ip, vm->regs.sp, vm->regs.fp}; \
        const int st = hot_int(&t, src, op); \
        vm->regs.ip = t.ip; \
        vm->regs.sp = t.sp; \
        return st ? fault(vm, st) : 0; \
    }
HANDLERS(X)
#undef X
