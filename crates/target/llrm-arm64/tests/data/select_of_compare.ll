target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

declare void @use(i64, i64)

define void @split(i64 %a, i64 %b) {
  %c = icmp ult i64 %a, %b
  %x = select i1 %c, i64 %a, i64 %b
  %y = select i1 %c, i64 0, i64 %a
  call void @use(i64 %x, i64 %y)
  ret void
}

define i64 @one(i64 %a, i64 %b) {
  %c = icmp ult i64 %a, %b
  %x = select i1 %c, i64 %a, i64 %b
  ret i64 %x
}
