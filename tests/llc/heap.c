#include <stdio.h>
#include <stdlib.h>
#include <string.h>
struct N{int k;struct N*l,*r;};
static struct N*ins(struct N*t,int k){if(!t){t=malloc(sizeof *t);t->k=k;t->l=t->r=0;return t;}if(k<t->k)t->l=ins(t->l,k);else t->r=ins(t->r,k);return t;}
static int height(struct N*t){if(!t)return 0;int a=height(t->l),b=height(t->r);return 1+(a>b?a:b);}
int main(void){struct N*t=0;for(int i=0;i<50;i++)t=ins(t,(i*37+11)%101);char*s=strdup("tree");printf("%d %s\n",height(t),s);free(s);return 0;}
