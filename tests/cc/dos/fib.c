#include "report.h"
static int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
int main(void) { report(fib(20)); return 0; }
