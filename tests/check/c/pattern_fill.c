// RUN: llrm-cc %s -O2 -march=i486 -S -o /dev/stdout
// A loop storing one word that no one byte repeats stays a loop: the pin has no memset_pattern off
// Darwin (LoopIdiomRecognize.cpp L1214 at d1106deb71cc), and loopidioms (b653584c96) does not form one.
// CHECK-LABEL: {{_fillw(@3)?}} proc
// CHECK-NOT: rep
// CHECK: mov word ptr _a[bx], 4660
// CHECK: jb
// CHECK: {{_fillw(@3)?}} endp
short a[100];
void fillw(void) { int i; for (i = 0; i < 100; i++) a[i] = 4660; }
