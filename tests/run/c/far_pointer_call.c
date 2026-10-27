// flags: -O2 -march=i486
// targets: x86-m16
extern void report(long v);
typedef void (__far *sink)(long);
static void __far note(long v) { report(v + 100); }
static void __far twice(long v) { report(v); report(v); }
static void call(sink f, long v) { f(v); }
int main(void)
{
    call(note, 7);
    call(twice, 3);
    return 0;
}
