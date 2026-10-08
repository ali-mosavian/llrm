// RUN: llrm-c %s {-O2 | -Os} -march=i486 -fno-inline-functions -S -o /dev/stdout
// Loops people write that are one string move, and look-alikes that are not.
// CHECK-LABEL: {{_copy(@3)?}} proc
// CHECK: rep movsd
// CHECK-NOT: std
// CHECK: {{_copy(@3)?}} endp
// CHECK-LABEL: {{_scroll_up(@3)?}} proc
// CHECK: rep movsd
// CHECK-NOT: std
// CHECK: {{_scroll_up(@3)?}} endp
// CHECK-LABEL: {{_scroll_down(@3)?}} proc
// CHECK: std
// CHECK: rep movsd
// CHECK: cld
// CHECK: {{_scroll_down(@3)?}} endp
// CHECK-LABEL: {{_palette(@3)?}} proc
// CHECK: rep stosw
// CHECK: {{_palette(@3)?}} endp
// CHECK-LABEL: {{_block(@3)?}} proc
// CHECK: rep movsd
// CHECK: {{_block(@3)?}} endp
// CHECK-LABEL: {{_smear(@3)?}} proc
// CHECK-NOT: rep
// CHECK: {{_smear(@3)?}} endp
// CHECK-LABEL: {{_changed(@3)?}} proc
// CHECK-NOT: rep
// CHECK: {{_changed(@3)?}} endp
// CHECK-LABEL: {{_two_stores(@3)?}} proc
// CHECK-NOT: rep
// CHECK: {{_two_stores(@3)?}} endp
// CHECK-LABEL: {{_strides(@3)?}} proc
// CHECK-NOT: rep
// CHECK: {{_strides(@3)?}} endp
// CHECK-LABEL: {{_written(@3)?}} proc
// CHECK-NOT: rep
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
