// flags: -O1 | -O2 | -Os
// targets: x86-m32
// ALU ops and tests on AL/AX/EAX take the short accumulator opcodes (04 ib, a8 ib, 66 05 iw, a9 id). A wrong operand size there is silent
// in byte counts, so each result is reported from high-bit inputs.
extern void report(long value);

#define BYTE(name, op, imm) __attribute__((noinline)) unsigned char name(unsigned char x) { return x op imm; }
BYTE(add8, +, 0x85)
BYTE(or8, |, 0x91)
BYTE(and8, &, 0xf3)
BYTE(xor8, ^, 0x5a)
BYTE(sub8, -, 0xc9)
#define WORD(name, op, imm) __attribute__((noinline)) unsigned short name(unsigned short x) { return x op imm; }
WORD(add16, +, 0x8765)
WORD(or16, |, 0x9100)
WORD(and16, &, 0xf3f3)
WORD(xor16, ^, 0x5a5a)
WORD(sub16, -, 0xc987)
#define LONG(name, op, imm) __attribute__((noinline)) unsigned name(unsigned x) { return x op imm; }
LONG(add32, +, 0x87654321u)
LONG(or32, |, 0x91000000u)
LONG(and32, &, 0xf3f3f3f3u)
LONG(xor32, ^, 0x5a5a5a5au)
LONG(sub32, -, 0xc9876543u)

// A value returned by a call is in AL, AX or EAX: the operand the short forms need.
__attribute__((noinline)) unsigned char pick8(unsigned v) { return (unsigned char)v; }
__attribute__((noinline)) unsigned short pick16(unsigned v) { return (unsigned short)v; }
__attribute__((noinline)) unsigned pick32(unsigned v) { return v; }
__attribute__((noinline)) int test8(unsigned v) { return (pick8(v) & 0x80) ? 17 : 3; }
__attribute__((noinline)) int test16(unsigned v) { return (pick16(v) & 0x8001) ? 19 : 5; }
__attribute__((noinline)) int test32(unsigned v) { return (pick32(v) & 0x80000001u) ? 23 : 7; }
__attribute__((noinline)) int cmp8(unsigned v) { return pick8(v) == 0xa5; }
__attribute__((noinline)) int cmp16(unsigned v) { return pick16(v) == 0xa5a5; }
__attribute__((noinline)) int cmp32(unsigned v) { return pick32(v) == 0xa5a5a5a5u; }
__attribute__((noinline)) int ops8(unsigned v) { unsigned char r = pick8(v); return (unsigned char)((r + 0x85) ^ 0x5a) | (r & 0xf3) << 8; }
__attribute__((noinline)) int ops16(unsigned v) { unsigned short r = pick16(v); return (unsigned short)((r + 0x8765) ^ 0x5a5a) + (r | 0x9100); }

int main(void)
{
    volatile unsigned inputs[] = {0u, 1u, 0x7fu, 0x80u, 0xffu, 0x8000u, 0xa5a5u, 0xffffu, 0x80000000u, 0xa5a5a5a5u, 0xffffffffu};
    long sum = 0;
    for (unsigned i = 0; i < sizeof inputs / sizeof inputs[0]; ++i) {
        unsigned v = inputs[i];
        sum = sum * 31 + add8(v) + or8(v) * 3 + and8(v) * 5 + xor8(v) * 7 + sub8(v) * 11;
        sum = sum * 31 + add16(v) + or16(v) * 3 + and16(v) * 5 + xor16(v) * 7 + sub16(v) * 11;
        sum = sum * 31 + (long)(add32(v) ^ or32(v)) + (long)(and32(v) ^ xor32(v)) + (long)sub32(v);
        sum = sum * 31 + test8(v) + test16(v) * 2 + test32(v) * 4 + cmp8(v) * 8 + cmp16(v) * 16 + cmp32(v) * 32 + ops8(v) + ops16(v);
        report(sum);
    }
    return 0;
}
