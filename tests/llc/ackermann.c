#include <stdio.h>
static int ack(int m,int n){return m==0?n+1:n==0?ack(m-1,1):ack(m-1,ack(m,n-1));}
static long gcd(long a,long b){return b?gcd(b,a%b):a;}
int main(void){printf("%d %ld\n",ack(2,3),gcd(1071,462));return 0;}
