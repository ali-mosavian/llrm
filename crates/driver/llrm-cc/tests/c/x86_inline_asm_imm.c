/* Constants in inline asm: `i`, `n`, `I`..`O`, `e`, `Z`, a set with a register (`ir`), and `%c`. */
static int shift(int x) { __asm__("shll %1, %0" : "+r"(x) : "I"(3)); return x; }
static int mask(int x) { __asm__("andl %1, %0" : "+r"(x) : "K"(-16)); return x; }
static int plus_n(int x) { __asm__("addl %1, %0" : "+r"(x) : "n"(1000)); return x; }
static int plus_i(int x) { __asm__("addl %1, %0" : "+r"(x) : "i"(77)); return x; }
static long plus_e(long x) { __asm__("add %1, %0" : "+r"(x) : "e"(-5L)); return x; }
static unsigned plus_z(unsigned x) { __asm__("addl %1, %0" : "+r"(x) : "Z"(0xfffffff0u)); return x; }
static int set_const(int x) { __asm__("andl %1, %0" : "+r"(x) : "ir"(255)); return x; }
static int set_var(int x, int m) { __asm__("andl %1, %0" : "+r"(x) : "ir"(m)); return x; }
static int bare(int x) { __asm__("addl $%c1, %0" : "+r"(x) : "i"(9)); return x; }
static int low(int x) { __asm__("andl %1, %0" : "+r"(x) : "M"(3)); return x; }
static int port_free(int x) { __asm__("orl %1, %0" : "+r"(x) : "N"(200)); return x; }
static int octal(int x) { __asm__("addl %1, %0" : "+r"(x) : "O"(100)); return x; }

int main(void)
{
    int bad = 0;
    bad += shift(5) != 40;
    bad += mask(0x1234) != 0x1230;
    bad += plus_n(1) != 1001;
    bad += plus_i(1) != 78;
    bad += plus_e(10) != 5;
    bad += plus_z(32) != 16;
    bad += set_const(0x1ff) != 0xff;
    bad += set_var(0x1ff, 0x0f) != 0x0f;
    bad += bare(1) != 10;
    bad += low(7) != 3;
    bad += port_free(1) != 201;
    bad += octal(1) != 101;
    return bad;
}
