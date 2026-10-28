target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@count = global i32 0, align 4

declare void @use(i32)

define void @bump() {
  %a = load i32, ptr @count, align 4
  call void @use(i32 %a)
  %b = load i32, ptr @count, align 4
  %c = add i32 %b, 1
  store i32 %c, ptr @count, align 4
  ret void
}
