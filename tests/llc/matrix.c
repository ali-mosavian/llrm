#include <stdio.h>
static int a[8][8],b[8][8],c[8][8];
int main(void){
  for(int i=0;i<8;i++)for(int j=0;j<8;j++){a[i][j]=i+j;b[i][j]=i-j;}
  for(int i=0;i<8;i++)for(int j=0;j<8;j++){int s=0;for(int k=0;k<8;k++)s+=a[i][k]*b[k][j];c[i][j]=s;}
  long t=0;for(int i=0;i<8;i++)for(int j=0;j<8;j++)t+=c[i][j]*(i*8+j);
  printf("%ld %d\n",t,c[3][5]);return 0;}
