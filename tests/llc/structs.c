#include <stdio.h>
struct P{int x;int y;long z;};
static struct P mk(int x,int y){struct P p={x,y,(long)x*y};return p;}
static long dot(const struct P*a,const struct P*b){return (long)a->x*b->x+(long)a->y*b->y+a->z*b->z;}
int main(void){struct P a=mk(3,4),b=mk(5,6);printf("%ld %d\n",dot(&a,&b),a.y);return 0;}
