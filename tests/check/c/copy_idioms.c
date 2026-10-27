// RUN: llrm-c %s {-O2 | -Os} -march=i486 -fno-inline-functions -S -o /dev/stdout
// Loops people write that are one string move, and look-alikes that are not.
// DEVIATE: at -Os a copy whose length is known only at run time stays a loop when the split `rep movs` is more bytes
// (no libc memcpy to call; loopidiom `variable_copy_pays`, 08ceb5c21): copy, scroll_up, scroll_down, block and the copy of two_stores are loops there.
// CHECK-LABEL: {{_copy(@3)?}} proc
// CHECK[-O2]: rep movsd
// CHECK[-Os]: dec ax
// CHECK-NOT: std
// CHECK: {{_copy(@3)?}} endp
// CHECK-LABEL: {{_scroll_up(@3)?}} proc
// CHECK[-O2]: rep movsd
// CHECK[-Os]: dec ax
// CHECK-NOT: std
// CHECK: {{_scroll_up(@3)?}} endp
// CHECK-LABEL: {{_scroll_down(@3)?}} proc
// CHECK[-O2]: std
// CHECK[-O2]: rep movsd
// CHECK[-O2]: cld
// CHECK[-Os]: dec ax
// CHECK: {{_scroll_down(@3)?}} endp
// CHECK-LABEL: {{_palette(@3)?}} proc
// DEVIATE: the pin leaves a variable 16-bit fill a loop (no memset_pattern off Darwin, LoopIdiomRecognize.cpp L1214 at
// d1106deb71cc); llrm forms `rep stosw` where the target's table prices it under the loop: clocks at speed, bytes at -Os
// (loopidioms.rs `_pays`). The check takes either, the loop being the -Os answer.
// CHECK: {{rep stosw|dec ax}}
// CHECK: {{_palette(@3)?}} endp
// CHECK-LABEL: {{_block(@3)?}} proc
// CHECK[-O2]: rep movsd
// CHECK[-Os]: dec dx
// CHECK: {{_block(@3)?}} endp
// CHECK-LABEL: {{_smear(@3)?}} proc
// CHECK-NOT: rep
// CHECK: {{_smear(@3)?}} endp
// CHECK-LABEL: {{_changed(@3)?}} proc
// CHECK-NOT: rep
// CHECK: {{_changed(@3)?}} endp
// The pin (clang d1106deb71cc -O2 -m32 -march=i486) distributes it into llvm.memset (c) and llvm.memcpy (a <- b), as LoopIdiomRecognize does a loop of
// independent stores: two string moves, not none.
// At -Os the loop of two_stores stays one loop (loopidioms `fills_pay`, 6af0a022f): the copy keeps the zero store beside it. m16 copy_idioms.c -Os
// 281 -> 268 B; per element the loop is 8 cycles on the i486 table against 9 for `rep stosb` plus the copy loop (estimate, no bench).
// CHECK-LABEL: {{_two_stores(@3)?}} proc
// CHECK[-O2]: rep stos
// CHECK[-O2]: rep movs
// CHECK[-Os]: mov word ptr _c[bx], 0
// CHECK[-Os]: dec ax
// CHECK: {{_two_stores(@3)?}} endp
// CHECK-LABEL: {{_strides(@3)?}} proc
// CHECK-NOT: rep
// CHECK: {{_strides(@3)?}} endp
// The pin makes b = 0 and a = b two llvm.memset calls (the second store reads what the first wrote, which is a constant).
// CHECK-LABEL: {{_written(@3)?}} proc
// At -Os both stores stay in one loop (same `fills_pay` answer as two_stores).
// CHECK[-O2]: rep stos
// CHECK[-O2]: rep stos
// CHECK[-Os]: mov word ptr _a[bx], cx
// CHECK[-Os]: dec ax
// CHECK: {{_written(@3)?}} endp
short a[160], b[160], c[160], pal[64];
void copy(short n) { short i; for (i = 0; i < n; ++i) a[i] = b[i]; }
void scroll_up(short n) { short i; for (i = 0; i < n; ++i) a[i] = a[i + 16]; }
void scroll_down(short n) { short i; for (i = n - 1; i >= 0; --i) a[i + 16] = a[i]; }
void palette(short n, short w) { short i; for (i = 0; i < n; ++i) pal[i] = w; }
void block(short w) { short y, x; for (y = 0; y < 4; ++y) for (x = 0; x < w; ++x) a[y * 16 + x] = b[y * 20 + x]; }
void smear(short n) { short i; for (i = 0; i < n; ++i) a[i + 1] = a[i]; }
void changed(short n) { short i; for (i = 0; i < n; ++i) a[i] = b[i] + 1; }
void two_stores(short n) { short i; for (i = 0; i < n; ++i) { a[i] = b[i]; c[i] = 0; } }
void strides(short n) { short i; for (i = 0; i < n / 2; ++i) a[2 * i] = b[i]; }
void written(short n) { short i; for (i = 0; i < n; ++i) { b[i] = 0; a[i] = b[i]; } }
