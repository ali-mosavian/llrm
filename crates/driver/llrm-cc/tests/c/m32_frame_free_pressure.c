/* A bytecode loop: `vm` is used by every handler, and a handler keeps six values live. */
struct Frame { unsigned char tag; int a; int b; int pad[6]; };
struct Vm {
    struct Frame *fp; int *sp; unsigned char *ip; unsigned char *base;
    int cur; int saved; int flag; long long count; unsigned short r1, r2; int slot;
};
extern int stop(struct Vm *);
extern unsigned char *code_base;
void run(struct Vm *vm)
{
    unsigned char *ip = vm->ip;
    static void *const at[4] = { &&op_call, &&op_ret, &&op_jmp, &&op_end };
    goto *at[*ip];
op_call: {
        unsigned char *p = ip + 3;
        int target = *(int *)p;
        struct Frame *f = vm->fp;
        int ret = vm->saved;
        int off = (int)(ip + 7 - (code_base + vm->slot));
        f->tag = 0; f->a = ret; f->b = off;
        vm->fp = f + 1;
        vm->r2 = vm->r1;
        vm->flag = target; vm->saved = target;
        vm->count++;
        ip = code_base + vm->slot + *(int *)(code_base + vm->slot + target + 6);
        goto *at[*ip];
    }
op_ret: {
        struct Frame *f = vm->fp - 1;
        int ret = f->a; int off = f->b;
        vm->fp = f; vm->saved = ret; vm->r2 = vm->r1; vm->flag = ret;
        vm->count++;
        ip = code_base + vm->slot + off;
        goto *at[*ip];
    }
op_jmp: {
        int d = *(int *)(ip + 1);
        vm->cur = d; vm->count++; vm->r1 = vm->r2;
        ip = code_base + vm->slot + d + vm->saved;
        goto *at[*ip];
    }
op_end:
    stop(vm);
}
