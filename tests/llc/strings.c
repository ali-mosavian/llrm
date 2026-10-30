#include <stdio.h>
static unsigned len(const char*s){unsigned n=0;while(s[n])n++;return n;}
int main(void){
  char buf[32]; const char*t="hello, x86-64 world";
  unsigned n=len(t);
  for(unsigned i=0;i<=n;i++)buf[i]=t[n-i? n-i-1: n]; buf[n]=0;
  printf("%u %s\n",n,buf);
  return 0;
}
