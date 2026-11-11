void f(int);
static int last = -1, count;
void g0(void){last=0;count++;} void g1(void){last=1;count++;} void g2(void){last=2;count++;}
void g3(void){last=3;count++;} void g4(void){last=4;count++;} void g5(void){last=5;count++;}
void g6(void){last=6;count++;} void g7(void){last=7;count++;} void g8(void){last=8;count++;}
void g9(void){last=9;count++;}
static long sys3(long n,long a,long b,long c){register long d0 __asm__("d0")=n,d1 __asm__("d1")=a,d2 __asm__("d2")=b,d3 __asm__("d3")=c;__asm__ volatile("trap #0":"+d"(d0):"d"(d1),"d"(d2),"d"(d3):"memory");return d0;}
int main(void){
  for (int x = -3; x <= 12; x++) {
    int before = count;
    f(x);
    int inside = x >= 0 && x <= 9;
    if (count != before + inside) return 100 + x + 3;
    if (inside && last != x) return 50 + x;
  }
  return 0;
}
void _start(void){ int r = main(); sys3(1, r, 0, 0); for(;;); }
