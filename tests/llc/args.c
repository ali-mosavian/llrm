#include <stdio.h>
__attribute__((noinline)) static long f(long a,long b,long c,long d,long e,long g,long h,long i){return a+2*b+3*c+4*d+5*e+6*g+7*h+8*i;}
int main(void){printf("%ld\n",f(1,2,3,4,5,6,7,8));return 0;}
