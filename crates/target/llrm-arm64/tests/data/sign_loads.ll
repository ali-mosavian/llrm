target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

define i64 @word(ptr %p) {
  %v = load i32, ptr %p, align 4
  %w = sext i32 %v to i64
  ret i64 %w
}

define i64 @half(ptr %p) {
  %v = load i16, ptr %p, align 2
  %w = sext i16 %v to i64
  ret i64 %w
}

define i32 @byte(ptr %p) {
  %v = load i8, ptr %p, align 1
  %w = sext i8 %v to i32
  ret i32 %w
}

define i64 @also_read_narrow(ptr %p, ptr %q) {
  %v = load i32, ptr %p, align 4
  %w = sext i32 %v to i64
  store i32 %v, ptr %q, align 4
  ret i64 %w
}
