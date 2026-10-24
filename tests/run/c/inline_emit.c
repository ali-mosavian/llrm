// flags: -O0 | -O2 -march=i486
// targets: x86-m16
/* Inline code as raw bytes (`__emit__`): mov ax, 7 returns 7 in AX; the same stored through a local's place. */
extern void report(long v);
int seven(void)
{
    __emit__(0xB8, 0x07, 0x00);
}
int main(void)
{
    report(seven());
    report(seven() + 1);
    return 0;
}
