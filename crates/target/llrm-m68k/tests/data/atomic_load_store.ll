target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"
@flag = global i32 0, align 4
define i1 @shown() {
  %v = load atomic i32, ptr @flag monotonic, align 4
  %r = icmp sgt i32 %v, 0
  ret i1 %r
}
define void @clear() {
  store atomic i32 0, ptr @flag monotonic, align 4
  ret void
}
define i32 @bump() {
  %old = atomicrmw add ptr @flag, i32 1 monotonic
  ret i32 %old
}
define i1 @claim() {
  %r = cmpxchg ptr @flag, i32 0, i32 1 monotonic monotonic
  %ok = extractvalue { i32, i1 } %r, 1
  ret i1 %ok
}
