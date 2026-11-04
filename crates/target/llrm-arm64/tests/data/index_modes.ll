target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

define void @shifted(ptr %p, i64 %i, i16 %v) {
  %o = shl i64 %i, 1
  %a = getelementptr i8, ptr %p, i64 %o
  store i16 %v, ptr %a, align 2
  ret void
}

define i32 @signed(ptr %p, i32 %i) {
  %w = sext i32 %i to i64
  %a = getelementptr i32, ptr %p, i64 %w
  %v = load i32, ptr %a, align 4
  ret i32 %v
}

define i32 @unsigned(ptr %p, i32 %i) {
  %w = zext i32 %i to i64
  %a = getelementptr i32, ptr %p, i64 %w
  %v = load i32, ptr %a, align 4
  ret i32 %v
}

define i64 @index_read_twice(ptr %p, i32 %i) {
  %w = sext i32 %i to i64
  %a = getelementptr i32, ptr %p, i64 %w
  %v = load i32, ptr %a, align 4
  %x = sext i32 %v to i64
  %r = add i64 %x, %w
  ret i64 %r
}

define void @too_wide(ptr %p, i64 %i, i64 %v) {
  %o = shl i64 %i, 3
  %a = getelementptr i64, ptr %p, i64 %o
  store i64 %v, ptr %a, align 8
  ret void
}

define i32 @loaded_index(ptr %p, ptr %q) {
  %i = load i32, ptr %q, align 4
  %w = sext i32 %i to i64
  %a = getelementptr i32, ptr %p, i64 %w
  %v = load i32, ptr %a, align 4
  ret i32 %v
}

%triple = type { i64, i64, i64 }

define i64 @loaded_index_into_a_record_array(ptr %p, ptr %q) {
  %i = load i32, ptr %q, align 4
  %w = sext i32 %i to i64
  %a = getelementptr %triple, ptr %p, i64 %w
  %v = load i64, ptr %a, align 8
  ret i64 %v
}
