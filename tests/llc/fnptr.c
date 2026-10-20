#include <stdio.h>
typedef int (*op)(int,int);
static int add(int a,int b){return a+b;} static int mul(int a,int b){return a*b;} static int sub(int a,int b){return a-b;}
static op table[3]={add,mul,sub};
__attribute__((noinline)) static int apply(op f,int a,int b){return f(a,b);}
int main(void){int s=0;for(int i=0;i<3;i++)s=s*7+apply(table[i],6,i+2);printf("%d\n",s);return 0;}
