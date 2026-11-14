target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

@v = global i16 0

declare void @use(i32)

define void @f() {
  %x = load volatile i16, ptr @v
  %w = zext nneg i16 %x to i32
  %c = icmp sgt i16 %x, 0
  br i1 %c, label %t, label %e
t:
  call void @use(i32 %w)
  ret void
e:
  ret void
}
