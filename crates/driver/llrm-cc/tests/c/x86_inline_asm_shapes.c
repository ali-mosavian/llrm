/* Flat asm blocks of each shape, 32- and 64-bit alike. main returns how many
   results are wrong. */
static int add1(int x, int y) { __asm__("addl %1, %0" : "+r"(x) : "r"(y)); return x; }
static int fixed(int x, int y) { int r; __asm__("subl %%ecx, %%eax" : "=a"(r) : "a"(x), "c"(y)); return r; }
static int sub1(int x, int y) { __asm__("subl %1, %0" : "+r"(x) : "r"(y)); return x; }
static int memory(int x, int y) { int m = y; __asm__("addl %1, %0" : "+m"(m) : "r"(x)); return m; }
static int two(int x, int y) {
  int s, d;
  __asm__("movl %%eax, %%edx\n\taddl %%ecx, %%eax\n\tsubl %%ecx, %%edx" : "=a"(s), "=d"(d) : "0"(x), "c"(y));
  return s * 10 + d;
}
static int swap_in_out(int x, int y) { __asm__("xchgl %0, %1" : "+r"(x), "+r"(y)); return x * 10 + y; }
int main(void) {
  return (add1(3, 4) != 7) + (fixed(3, 4) != -1) + (sub1(3, 4) != -1) + (memory(3, 4) != 7) + (two(9, 4) != 135) +
         (swap_in_out(3, 4) != 43);
}
