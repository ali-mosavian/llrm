target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

@xs = internal constant [4 x i32] [i32 1, i32 2, i32 3, i32 4]

declare void @g(i32)

; Three IR values for one address, read in three blocks: three copies of it.
define void @f(i32 %i, i1 %c) {
entry:
  %a = getelementptr inbounds nuw [4 x i8], ptr @xs, i32 %i
  %b = getelementptr inbounds nuw [4 x i8], ptr @xs, i32 %i
  %d = getelementptr inbounds nuw [4 x i8], ptr @xs, i32 %i
  br i1 %c, label %l, label %r

l:
  %x = load i32, ptr %a
  call void @g(i32 %x)
  br label %m

r:
  %y = load i32, ptr %b
  call void @g(i32 %y)
  br label %m

m:
  %z = load i32, ptr %d
  call void @g(i32 %z)
  ret void
}
