int table_jump(int);
int slot_jump(int, int);
int g1(void) { return 11; }
int g2(void) { return 22; }
int g3(void) { return 33; }
static long sys3(long n,long a,long b,long c){register long d0 __asm__("d0")=n,d1 __asm__("d1")=a,d2 __asm__("d2")=b,d3 __asm__("d3")=c;__asm__ volatile("trap #0":"+d"(d0):"d"(d1),"d"(d2),"d"(d3):"memory");return d0;}
int main(void){
  static const int want[3] = {11, 22, 33};
  for (int x = 0; x < 3; x++)
    if (table_jump(x) != want[x]) return 10 + x;
  for (int c = 0; c < 2; c++)
    if (slot_jump(c, 5) != (c ? 6 : 10)) return 20 + c;
  return 0;
}
void _start(void){ int r = main(); sys3(1, r, 0, 0); for(;;); }
