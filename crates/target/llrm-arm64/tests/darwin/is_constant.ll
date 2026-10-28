; llvm.is.constant is 1 for a constant operand and 0 for any other value once nothing is left to fold (clang -O0 keeps it).
source_filename = "is_constant.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@.str = private unnamed_addr constant [7 x i8] c"%d %d\0A\00", align 1
@g = internal global i32 9, align 4

define i32 @main() {
  %v = load volatile i32, ptr @g, align 4
  %a = call i1 @llvm.is.constant.i32(i32 5)
  %b = call i1 @llvm.is.constant.i32(i32 %v)
  %x = zext i1 %a to i32
  %y = zext i1 %b to i32
  %p = call i32 (ptr, ...) @printf(ptr @.str, i32 %x, i32 %y)
  ret i32 0
}

declare i1 @llvm.is.constant.i32(i32)
declare i32 @printf(ptr, ...)
