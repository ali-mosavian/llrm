#include <stdio.h>
#include <stdlib.h>
#include <x86intrin.h>
extern int bench_main_(void);
static long last; static int got;
void report(long v){ if(!got){last=v;got=1;} }
int main(int argc, char **argv){
  int reps = argc>1?atoi(argv[1]):31; unsigned long long t[512];
  for(int i=0;i<reps;i++){ unsigned long long a=__rdtsc(); bench_main_(); t[i]=__rdtsc()-a; }
  for(int i=1;i<reps;i++){ unsigned long long k=t[i]; int j=i-1; while(j>=0&&t[j]>k){t[j+1]=t[j];j--;} t[j+1]=k; }
  if(reps>1) printf("%d %llu %llu %llu\n",(int)last,t[0],t[reps/2],t[reps-1]);
  return 0;
}
