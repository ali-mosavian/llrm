// flags: -O2 | -Os
/* A string one function reads through its name and another reads through a pointer in DS: at m16 the string stays in
   DS, where that pointer reaches it (a copy in the code segment printed garbage through the pointer). */
extern void report(long value);
const char *names[2] = {"abc", "def"};
__attribute__((noinline)) long first(int i) { return "abc"[i]; }
__attribute__((noinline)) long second(int i) { return names[0][i]; }
int main(void)
{
    int i;
    for (i = 0; i < 3; i++) {
        report(first(i));
        report(second(i));
    }
    return 0;
}
