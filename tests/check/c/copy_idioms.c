// RUN: llrm-c %s -O2 --cpu 486 -fno-inline-functions -S -o /dev/stdout
// Loops people write that are one string move, and look-alikes that are not.
// CHECK-LABEL: _copy proc
// CHECK: rep movsd
// CHECK-NOT: std
// CHECK: _copy endp
// CHECK-LABEL: _scroll_up proc
// CHECK: rep movsd
// CHECK-NOT: std
// CHECK: _scroll_up endp
// CHECK-LABEL: _scroll_down proc
// CHECK: std
// CHECK: rep movsd
// CHECK: cld
// CHECK: _scroll_down endp
// CHECK-LABEL: _palette proc
// CHECK: rep stosw
// CHECK: _palette endp
// CHECK-LABEL: _block proc
// CHECK: rep movsd
// CHECK: _block endp
// CHECK-LABEL: _smear proc
// CHECK-NOT: rep
// CHECK: _smear endp
// CHECK-LABEL: _changed proc
// CHECK-NOT: rep
// CHECK: _changed endp
// CHECK-LABEL: _two_stores proc
// CHECK-NOT: rep
// CHECK: _two_stores endp
// CHECK-LABEL: _strides proc
// CHECK-NOT: rep
// CHECK: _strides endp
// CHECK-LABEL: _written proc
// CHECK-NOT: rep
// CHECK: _written endp
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
