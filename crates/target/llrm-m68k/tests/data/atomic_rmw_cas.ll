target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"
define i32 @add(ptr %p, i32 %v) {
  %r = atomicrmw add ptr %p, i32 %v seq_cst
  ret i32 %r
}
define void @sub(ptr %p, i32 %v) {
  %r = atomicrmw sub ptr %p, i32 %v seq_cst
  ret void
}
define i32 @xchg(ptr %p, i32 %v) {
  %r = atomicrmw xchg ptr %p, i32 %v monotonic
  ret i32 %r
}
define i32 @nand(ptr %p, i32 %v) {
  %r = atomicrmw nand ptr %p, i32 %v seq_cst
  ret i32 %r
}
define i32 @cas(ptr %p, i32 %e, i32 %n) {
  %r = cmpxchg ptr %p, i32 %e, i32 %n seq_cst seq_cst
  %v = extractvalue { i32, i1 } %r, 0
  ret i32 %v
}
define i1 @swapped(ptr %p, i32 %e, i32 %n) {
  %r = cmpxchg ptr %p, i32 %e, i32 %n seq_cst seq_cst
  %v = extractvalue { i32, i1 } %r, 1
  ret i1 %v
}
define i8 @umaxb(ptr %p, i8 %v) {
  %r = atomicrmw umax ptr %p, i8 %v acq_rel
  ret i8 %r
}
