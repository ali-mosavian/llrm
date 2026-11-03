target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
declare void @a()
declare void @b()
define void @f(i32 %x) {
  %c = icmp ult i32 %x, 32
  %s = select i1 %c, i1 %c, i1 false
  br i1 %s, label %ta, label %tb
ta:
  call void @a()
  ret void
tb:
  call void @b()
  ret void
}
