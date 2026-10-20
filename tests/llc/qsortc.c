#include <stdio.h>
#include <stdlib.h>
static int cmp(const void*a,const void*b){int x=*(const int*)a,y=*(const int*)b;return (x>y)-(x<y);}
int main(void){int a[12]={9,3,7,1,8,2,12,5,11,4,10,6};qsort(a,12,sizeof a[0],cmp);for(int i=0;i<12;i++)printf("%d ",a[i]);printf("\n");return a[0]+a[11];}
