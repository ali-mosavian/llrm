#include <stdio.h>
volatile double vlimit = 7.3; volatile float vstep = 0.7f;
int main(void) {
    double x = 0, lim = vlimit; float s = vstep; volatile float acc = 0;  /* stored each step: x87 would keep it wider */ int n = 0, pos = 0;
    while (x < lim) { x += s; n++; acc += (x > 3.0 ? s : -s); pos += x > 3.0 ? 1 : 0; }
    printf("%d %d %ld %ld\n", n, pos, (long)(x * 1000), (long)(acc * 1000));
    return 0;
}
