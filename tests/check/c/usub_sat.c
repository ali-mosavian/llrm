// RUN: llrm-cc %s -m16 -Os -S -o /dev/stdout
// RUN: llrm-cc %s -m16 -Os --isel=dag -S -o /dev/stdout
// RUN: llrm-cc %s -m32 -mabi=sysv -Os -march=i486 -S -o /dev/stdout
// RUN: llrm-cc %s -m32 -mabi=sysv -Os -march=i486 --isel=dag -S -o /dev/stdout
// `n > k ? n - k : 0` is `umax(n, k) - k`, which InstCombine makes `usub.sat` as the pin does. The backend had no `usub.sat`
// (--isel=old stopped at "@llvm.usub.sat", --isel=dag panicked in assemble): this keeps the idiom compiling on both selectors.
unsigned room(unsigned n, unsigned k)
{
    return n > k ? n - k : 0;
}
// CHECK-LABEL: _room
// CHECK: sub
