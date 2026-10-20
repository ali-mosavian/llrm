#include <stdio.h>
#include <stdarg.h>
static int sum(int n,...){va_list ap;va_start(ap,n);int s=0;for(int i=0;i<n;i++)s+=va_arg(ap,int);va_end(ap);return s;}
static long lsum(int n,...){va_list ap;va_start(ap,n);long s=0;for(int i=0;i<n;i++)s+=va_arg(ap,long);va_end(ap);return s;}
int main(void){printf("%d %ld\n",sum(5,1,2,3,4,5),lsum(8,1L,2L,3L,4L,5L,6L,7L,8L));return 0;}
