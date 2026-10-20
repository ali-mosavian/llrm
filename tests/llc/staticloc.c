#include <stdio.h>
static int counter(void){static int n;return ++n;}
struct S{char c;short s;int i;long l;char t[5];};
static struct S g={1,2,3,4,{5,6,7,8,9}};
int main(void){int s=0;for(int i=0;i<5;i++)s+=counter();struct S l=g;l.i+=s;printf("%d %d %d %ld %d\n",s,l.i,l.c,l.l,l.t[4]);return 0;}
