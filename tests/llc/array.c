#include <stdio.h>
int a[64];
int main(void){
  for(int i=0;i<64;i++)a[i]=i*i%17;
  long s=0;
  for(int i=0;i<64;i++)s+=a[i]*(i+1);
  printf("%ld\n",s);
  return 0;
}
