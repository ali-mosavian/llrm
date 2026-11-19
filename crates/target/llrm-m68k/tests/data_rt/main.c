int sum(void), letter(int), word_value(void), through_table(void), bump(void);
static long sys3(long n,long a,long b,long c){register long d0 __asm__("d0")=n,d1 __asm__("d1")=a,d2 __asm__("d2")=b,d3 __asm__("d3")=c;__asm__ volatile("trap #0":"+d"(d0):"d"(d1),"d"(d2),"d"(d3):"memory");return d0;}
int main(void){
  if (sum() != 10 - 20 + 300 - 4000) return 1;
  if (letter(0) != 'h' || letter(4) != 'o' || letter(5) != 0) return 2;
  if (word_value() != -2) return 3;
  if (through_table() != 10 + 7 + 300) return 4;
  if (bump() != 5 || bump() != 10) return 5;
  return 0;
}
void _start(void){ int r = main(); sys3(1, r, 0, 0); for(;;); }
