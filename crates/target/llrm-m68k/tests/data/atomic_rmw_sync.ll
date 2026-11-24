target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"
define i32 @add(ptr %p, i32 %v) {
  %r = atomicrmw add ptr %p, i32 %v seq_cst
  ret i32 %r
}
