// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* An argument in EAX survives the float control word saved at entry: the save wrote AX before the argument was read, and
   conv(7, 1.5f) printed the control word's arithmetic instead of 7003. */
extern void report(long value);
long conv(short n, float x) { return (long)(x * 2.0f) + n * 1000; }
int main(void) { report(conv(7, 1.5f)); return 0; }
