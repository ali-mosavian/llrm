// flags: -O2 -march=i486 -mabi=watcom
// link: watcom_far_sum_callee.c
/* `*p + a + b` with a far pointer in AX:DX, a in BX and b in CX under -mabi=watcom. The sum may be made as (*p + a) + b or as *p + (a + b),
   clobbering CX; the caller reads b again after the call, so a convention that keeps CX would show here. */
extern void report(long value);
extern int far_pointer(char far *p, int a, int b);

#define MK_FP(seg, off) ((void far *)(((unsigned long)(seg) << 16) | (off)))

int main(void)
{
    char far *p = (char far *)MK_FP(0x5000, 0);
    int b = 234;

    *p = 251; /* char is unsigned here */
    report(far_pointer(p, 1000, b));
    report(far_pointer(p, -7, 3) + 3);
    report(far_pointer(p, 1000, b) + b);
    return 0;
}
