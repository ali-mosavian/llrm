#include <stdio.h>
static unsigned char s[10000];
int main(void){
  int n=0;
  for(int i=2;i<10000;i++){ if(!s[i]){ n++; for(int j=i*2;j<10000;j+=i)s[j]=1; } }
  printf("%d\n",n);
  return 0;
}
