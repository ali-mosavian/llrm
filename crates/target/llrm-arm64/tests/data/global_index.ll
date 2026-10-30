target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"

@table = global [256 x i32] zeroinitializer
@bytes = global [256 x i8] zeroinitializer

define i32 @pick(i64 %i) {
  %p = getelementptr [256 x i32], ptr @table, i64 0, i64 %i
  %v = load i32, ptr %p
  %q = getelementptr [256 x i8], ptr @bytes, i64 0, i64 %i
  %b = load i8, ptr %q
  %w = zext i8 %b to i32
  %s = add i32 %v, %w
  ret i32 %s
}
