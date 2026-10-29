target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

declare void @use(i64, i64)
declare void @other()

define void @split(i64 %a, i64 %b, ptr %out) {
  %d = sub nuw i64 %a, %b
  %c = icmp ult i64 %d, 32
  %x = select i1 %c, i64 %a, i64 %b
  %y = select i1 %c, i64 0, i64 %d
  store volatile i64 %x, ptr %out
  br i1 %c, label %whole, label %part
whole:
  call void @use(i64 %y, i64 %x)
  ret void
part:
  call void @other()
  call void @use(i64 %x, i64 %d)
  ret void
}
