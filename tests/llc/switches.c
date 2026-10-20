#include <stdio.h>
static int f(int x){switch(x){case 0:return 10;case 1:return 21;case 2:return 33;case 3:return 47;case 4:return 52;case 5:return 68;default:return -1;}}
int main(void){int s=0;for(int i=-1;i<8;i++)s+=f(i);printf("%d\n",s);return 0;}
