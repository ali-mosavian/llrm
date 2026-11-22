target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"
define i32 @t(ptr %p) {
  %v = load i32, ptr %p, align 2
  %c = icmp eq i32 %v, 0
  %r = select i1 %c, i32 1, i32 2
  ret i32 %r
}
