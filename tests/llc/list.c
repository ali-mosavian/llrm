#include <stdio.h>
#include <stdint.h>
struct N{int v;struct N*next;};
static struct N pool[16];
int main(void){
  struct N*h=0;
  for(int i=0;i<16;i++){pool[i].v=i*3+1;pool[i].next=h;h=&pool[i];}
  uint32_t s=0;for(struct N*p=h;p;p=p->next)s=s*3+(uint32_t)p->v;
  printf("%lu\n",(unsigned long)s);
  return 0;
}
