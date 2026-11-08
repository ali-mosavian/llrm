/* `g`: a select in a loop, of 1 and 2 on the 486, which has no cmov (bintree -O3: `xor; cmp; setle; ...` 4+1 clocks 2492 times
   against gcc's `jge` 1-3). `f`: four zero stores in straight-line code (nbody -O1: 8 B each as `mov dword [m],0`). */
int g(int *t, int n) { int r = 0; for (int i = 0; i < n; i++) { int s = t[i] < i ? 1 : 2; r += t[s] + t[s + 4]; } return r; }
/* the same select run once: the smaller `setcc` form stays (sfm16_dbg -O3 grew 448 B for no clock otherwise) */
int h(int *t, int a, int b) { int s = a < b ? 1 : 2; return t[s] + t[s + 4]; }
int f(int *a, int *b) { a[0] = 0; a[1] = 0; a[2] = 0; a[3] = 0; b[0] = 7; return b[1]; }
