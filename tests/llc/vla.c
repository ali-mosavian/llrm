#include <stdio.h>
#include <string.h>
static int f(int n){int a[n];for(int i=0;i<n;i++)a[i]=i*i;int s=0;for(int i=0;i<n;i++)s+=a[i];return s;}
static void rev(char*s){size_t n=strlen(s);for(size_t i=0;i<n/2;i++){char t=s[i];s[i]=s[n-1-i];s[n-1-i]=t;}}
int main(void){char buf[20];strcpy(buf,"abcdefgh");rev(buf);printf("%d %s %d\n",f(10),buf,f(3));return 0;}
