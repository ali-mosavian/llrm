// flags: -O2 -march=i486 -m32
/* An unmarked pointer is the target's native near one in every memory model, 4 bytes on m32, as is __far. Large model's default data pointers were far (4). */
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
