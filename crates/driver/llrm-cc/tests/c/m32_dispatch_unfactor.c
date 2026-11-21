/* A computed-goto interpreter with more than 16 handlers: its one dispatch block has more than 16
   predecessors and successors with a phi, which pre-allocation tail duplication leaves factored. */
extern void sink(int);
typedef struct { const unsigned char *ip; int *sp; int acc; } R;
int run(const unsigned char *code, int *stack, int *count)
{
    static const void *const tab[] = {&&op0, &&op1, &&op2, &&op3, &&op4, &&op5, &&op6, &&op7, &&op8, &&op9, &&op10, &&op11, &&op12, &&op13, &&op14, &&op15, &&op16, &&op17, &&op18, &&op19, &&halt };
    R r = { code, stack, 0 };
#define NEXT do { ++*count; goto *tab[*r.ip]; } while (0)
    NEXT;
op0: r.acc = r.acc * 3 + r.sp[-1]; r.sp[0] = r.acc ^ 1; r.sp += 1; r.ip += 1; NEXT;
op1: r.acc = r.acc * 4 + r.sp[-2]; r.sp[1] = r.acc ^ 2; r.sp += 2; r.ip += 2; NEXT;
op2: r.acc = r.acc * 5 + r.sp[-3]; r.sp[2] = r.acc ^ 3; r.sp += 1; r.ip += 3; NEXT;
op3: r.acc = r.acc * 6 + r.sp[-1]; r.sp[3] = r.acc ^ 4; r.sp += 2; r.ip += 1; NEXT;
op4: r.acc = r.acc * 7 + r.sp[-2]; r.sp[0] = r.acc ^ 5; r.sp += 1; r.ip += 2; NEXT;
op5: r.acc = r.acc * 8 + r.sp[-3]; r.sp[1] = r.acc ^ 6; r.sp += 2; r.ip += 3; NEXT;
op6: r.acc = r.acc * 9 + r.sp[-1]; r.sp[2] = r.acc ^ 7; r.sp += 1; r.ip += 1; NEXT;
op7: r.acc = r.acc * 10 + r.sp[-2]; r.sp[3] = r.acc ^ 8; r.sp += 2; r.ip += 2; NEXT;
op8: r.acc = r.acc * 11 + r.sp[-3]; r.sp[0] = r.acc ^ 9; r.sp += 1; r.ip += 3; NEXT;
op9: r.acc = r.acc * 12 + r.sp[-1]; r.sp[1] = r.acc ^ 10; r.sp += 2; r.ip += 1; NEXT;
op10: r.acc = r.acc * 13 + r.sp[-2]; r.sp[2] = r.acc ^ 11; r.sp += 1; r.ip += 2; NEXT;
op11: r.acc = r.acc * 14 + r.sp[-3]; r.sp[3] = r.acc ^ 12; r.sp += 2; r.ip += 3; NEXT;
op12: r.acc = r.acc * 15 + r.sp[-1]; r.sp[0] = r.acc ^ 13; r.sp += 1; r.ip += 1; NEXT;
op13: r.acc = r.acc * 16 + r.sp[-2]; r.sp[1] = r.acc ^ 14; r.sp += 2; r.ip += 2; NEXT;
op14: r.acc = r.acc * 17 + r.sp[-3]; r.sp[2] = r.acc ^ 15; r.sp += 1; r.ip += 3; NEXT;
op15: r.acc = r.acc * 18 + r.sp[-1]; r.sp[3] = r.acc ^ 16; r.sp += 2; r.ip += 1; NEXT;
op16: r.acc = r.acc * 19 + r.sp[-2]; r.sp[0] = r.acc ^ 17; r.sp += 1; r.ip += 2; NEXT;
op17: r.acc = r.acc * 20 + r.sp[-3]; r.sp[1] = r.acc ^ 18; r.sp += 2; r.ip += 3; NEXT;
op18: r.acc = r.acc * 21 + r.sp[-1]; r.sp[2] = r.acc ^ 19; r.sp += 1; r.ip += 1; NEXT;
op19: r.acc = r.acc * 22 + r.sp[-2]; r.sp[3] = r.acc ^ 20; r.sp += 2; r.ip += 2; NEXT;
halt: return r.acc;
}
