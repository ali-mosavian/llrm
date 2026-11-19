struct pad { char a; int b; char c; short d; char e; };
extern struct pad pad;
extern int z1[3], z2[3];
extern long long zl, longs[2];
extern short ints[4];
extern unsigned char bytes[5];
extern void *ptrs[3], *fptrs[2];
int touch_bss(int), abc_length(void), padsum(void), f(void), name(int);
static long sys3(long n,long a,long b,long c){register long d0 __asm__("d0")=n,d1 __asm__("d1")=a,d2 __asm__("d2")=b,d3 __asm__("d3")=c;__asm__ volatile("trap #0":"+d"(d0):"d"(d1),"d"(d2),"d"(d3):"memory");return d0;}
int main(void){
  /* 11: zero-initialised data is zero, and adjacent objects do not overlap */
  for (int i = 0; i < 3; i++) if (z1[i] || z2[i]) return 11;
  if (zl) return 11;
  touch_bss(1);
  if (z1[0] || z1[1] != 7 || z1[2] || z2[0] != 9 || z2[1] || z2[2] || zl) return 11;
  /* 12: initialised integers of every width, big-endian */
  if (ints[0] != 1 || ints[1] != -2 || ints[2] != 300 || ints[3] != -4000) return 12;
  if (longs[0] != -2 || longs[1] != 0x0102030405060708LL) return 12;
  if (bytes[0] != 1 || bytes[3] != 4 || bytes[4] != 0xFF) return 12;
  /* 13: string literals, private names, terminator */
  if (abc_length() != 3) return 13;
  if (((char *)ptrs[2])[0] != 'x' || ((char *)ptrs[2])[1] != 'y' || ((char *)ptrs[2])[2] != 0) return 13;
  /* 14: pointers to globals, with an offset, and to functions */
  if (ptrs[0] != (void *)ints || ptrs[1] != (void *)&ints[3]) return 14;
  if (((int (*)(void))fptrs[0])() != 1 || ((int (*)(void))fptrs[1])() != 2) return 14;
  /* 15: a struct with padding: gcc's layout and ours agree */
  if (pad.a != 1 || pad.b != 2 || pad.c != 3 || pad.d != 4 || pad.e != 5) return 15;
  if (padsum() != 15) return 15;
  /* 16: function-local statics */
  if (f() != 101 || f() != 102) return 16;
  if (name(0) != 'A' || name(1) != 'B') return 16;
  return 0;
}
void _start(void){ int r = main(); sys3(1, r, 0, 0); for(;;); }
