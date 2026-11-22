#include <stdio.h>
unsigned long long get(int x) { return (4ull << 48) | ((unsigned long long)x << 32); }
void op(void *C, unsigned char a, unsigned short b) { printf("op %d %d\n", a, b); }
void op2(void *C, unsigned char a) { printf("op2 %d\n", a); }
void op3(void *C, unsigned char a, unsigned short b) { printf("op3 %d %d\n", a, b); }
void f(void *, int);
int main(void) { f(0, 7); return 0; }
