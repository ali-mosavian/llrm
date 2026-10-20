#include <stdio.h>
int a=7,b=9; int arr[4]={1,2,3,4};
int *tab[3]={&a,&b,&arr[2]};
const char *names[]={"zero","one","two"};
int main(void){int s=0;for(int i=0;i<3;i++)s=s*10+*tab[i];printf("%d %s %s\n",s,names[1],names[2]);return s%100;}
