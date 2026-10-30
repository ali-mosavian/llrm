target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

@counter = internal global i32 0
@llvm.global_ctors = appending global [3 x { i32, ptr, ptr }] [{ i32, ptr, ptr } { i32 200, ptr @late, ptr null }, { i32, ptr, ptr } { i32 100, ptr @early, ptr null }, { i32, ptr, ptr } { i32 150, ptr null, ptr null }]

define internal void @early() {
  store i32 1, ptr @counter, align 4
  ret void
}

define internal void @late() {
  store i32 2, ptr @counter, align 4
  ret void
}

define i32 @get() {
  %v = load i32, ptr @counter, align 4
  ret i32 %v
}
