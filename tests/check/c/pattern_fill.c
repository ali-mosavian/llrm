// RUN: llrm-cc %s -O2 -march=i486 -S -o /dev/stdout
// A loop storing one word that no one byte repeats is `rep stosw` where the fill pays (loopidioms). The
// pin has no memset_pattern off Darwin (LoopIdiomRecognize.cpp L1214 at d1106deb71cc) and keeps the loop:
// DEVIATE, measured on the nib twin, 100 words, 12 executed instructions and 2 memory operands as the fill
// against 609 and 201 as the loop.
// CHECK-LABEL: {{_fillw(@3)?}} proc
// CHECK: mov {{ax|eax}}, 4660
// CHECK: rep stosw
// CHECK: {{_fillw(@3)?}} endp
short a[100];
void fillw(void) { int i; for (i = 0; i < 100; i++) a[i] = 4660; }
