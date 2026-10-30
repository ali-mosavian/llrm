target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"
%S = type { i32, i32, i32, i32 }
declare void @take(ptr byval(%S) align 4)
declare void @mk(ptr sret(%S) align 4)
@g = global %S zeroinitializer
define i32 @param(ptr byval(%S) align 4 %p) {
  %q = getelementptr %S, ptr %p, i32 0, i32 1
  %v = load i32, ptr %q
  ret i32 %v
}
define void @caller() {
  call void @take(ptr byval(%S) align 4 @g)
  ret void
}
define void @callsret(ptr %out) {
  call void @mk(ptr sret(%S) align 4 %out)
  ret void
}
define void @retsret(ptr sret(%S) align 4 %out) {
  store i32 1, ptr %out
  ret void
}
