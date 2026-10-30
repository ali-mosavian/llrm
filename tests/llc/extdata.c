#include <stdio.h>
#include <errno.h>
#include <stdlib.h>
int main(void){fputs("to stdout\n",stdout);fprintf(stderr,"to stderr %d\n",7);errno=0;long v=strtol("12x",0,10);printf("%ld %d\n",v,errno);return 0;}
