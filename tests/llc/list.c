#include <stdio.h>
struct N{int v;struct N*next;};
static struct N pool[16];
int main(void){
  struct N*h=0;
  for(int i=0;i<16;i++){pool[i].v=i*3+1;pool[i].next=h;h=&pool[i];}
  int s=0;for(struct N*p=h;p;p=p->next)s=s*3+p->v;
  printf("%d\n",s);
  return 0;
}
