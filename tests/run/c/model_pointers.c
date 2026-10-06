// flags: -O2 -march=i486
// targets: x86-code16
/* An unmarked pointer is the target's native near one in every memory model, 2 bytes here; __far is 4. Large model's default data pointers were far (4). */
extern void report(long value);
char buf[8];
int main(void)
{
    char *p = buf;
    char __far *q = (char __far *)buf;
    report(sizeof(p));
    report(sizeof(q));
    report(sizeof(buf));
    return 0;
}
